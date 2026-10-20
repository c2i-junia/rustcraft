"""Builds the sheep model, data/models/animated/Sheep.glb, voxel by voxel in Blender.

The sheep is made of 1/16 m voxels, one per pixel of a block texture, in six rigid parts
(body, head, four legs) and a tail. Its fleece has bevelled edges and is covered with 2x2
curls laid out like a checkerboard. Each voxel takes its colour from a 16x16 palette drawn
with nearest filtering. The colours are drawn with a fixed seed, so the output is
reproducible.

The clips move the parts as whole objects, with no skinning. They are Eat, Idle and Walk,
which the exporter sorts by name into glTF animations 0, 1 and 2, the indices that
client/src/mob/models.rs expects. Eat and Idle last 3 s, Walk 1 s, at 24 fps. The model
is in metres with its feet on the origin, and faces -Y in Blender, which is +Z in glTF.

Usage: blender --background --factory-startup --python scripts/generate_sheep.py -- [output.glb]
"""

import math
import random
import sys
import tempfile
from pathlib import Path

import bmesh
import bpy

ROOT = Path(__file__).resolve().parent.parent
ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = Path(ARGS[0]) if ARGS else ROOT / "data" / "models" / "animated" / "Sheep.glb"
VOX = 1 / 16
FPS = 24
PALETTE_SIZE = 16

WOOL = ["#e4ddcf", "#dcd4c4"]
CURL = ["#fbf9f4", "#f3efe6"]
SKIN, SKIN_DARK, HOOF, EYE, NOSE = "#dcae8e", "#c08f70", "#4b382c", "#1f1a1a", "#e08891"
LEGS = (("leg_FL", range(-5, -1), range(-7, -3)), ("leg_FR", range(1, 5), range(-7, -3)),
        ("leg_BL", range(-5, -1), range(3, 7)), ("leg_BR", range(1, 5), range(3, 7)))
TAIL = {(i, j, k) for i in (-1, 0) for j in (9, 10) for k in (12, 13, 14)}


# ---------------------------------------------------------------------------- the voxels

def box(cells, xr, yr, zr, colour):
    for i in xr:
        for j in yr:
            for k in zr:
                cells[(i, j, k)] = colour(i, j, k)


def sheep_cells():
    """Every part as {(x, y, z): colour}, in voxels; y points to the tail, z up."""
    rng = random.Random(21)
    wool = lambda *_: rng.choices(WOOL + CURL, weights=(3, 2, 1, 1))[0]

    body = {}
    box(body, range(-6, 6), range(-8, 9), range(7, 16), wool)
    box(body, range(-1, 1), range(9, 11), range(12, 15), wool)
    head = {}
    box(head, range(-3, 3), range(-14, -8), range(11, 19), lambda *_: SKIN)
    for (i, j, k) in list(head):
        if k == 18 or (k == 17 and j > -14) or (k >= 15 and j >= -10) or (j == -14 and k == 17):
            head[(i, j, k)] = wool()
    head[(-2, -14, 14)] = EYE
    head[(1, -14, 14)] = EYE
    for i in (-1, 0):
        head[(i, -14, 12)] = NOSE
        head[(i, -14, 11)] = SKIN_DARK
    for x in (-5, -4, 3, 4):
        head[(x, -12, 16)] = SKIN_DARK
    parts = {"body": body, "head": head}
    for name, xr, yr in LEGS:
        leg = {}
        box(leg, xr, yr, range(0, 7), lambda i, j, k: HOOF if k < 2 else (wool() if k >= 5 else SKIN))
        parts[name] = leg

    # Bevel the edges of the fleece, then grow 2x2 curls on every face but the belly
    for c in list(body):
        i, j, k = c
        if j <= 8 and sum((i in (-6, 5), j in (-8, 8), k in (7, 15))) >= 2:
            del body[c]
    for c in list(body):
        if c[1] > 8:
            continue
        i, j, k = c
        for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1)):
            n = (i + d[0], j + d[1], k + d[2])
            if n in body:
                continue
            u, v = (i, j) if d[2] else (j, k) if d[0] else (i, k)
            if ((u // 2) + (v // 2)) % 2 == 0:
                body[n] = rng.choice(CURL)
    for i in range(-3, 3):
        for j in range(-14, -8):
            if ((i // 2) + (j // 2)) % 2 == 0:
                head[(i, j, 19)] = rng.choice(CURL)

    parts["tail"] = {c: body.pop(c) for c in TAIL}
    for c in head:
        body.pop(c, None)
    return parts


def pivots(parts):
    """Where each part turns, in voxels: the neck, the hips, the root of the tail."""
    out = {"body": (0, 0, 11), "head": (0, -9, 14), "tail": (0, 9, 13.5)}
    for name, cells in parts.items():
        if name.startswith("leg"):
            xs, ys, zs = zip(*cells)
            out[name] = ((min(xs) + max(xs) + 1) / 2, (min(ys) + max(ys) + 1) / 2, max(zs) + 1)
    return out


# ---------------------------------------------------------------------------- the meshes

FACES = (
    ((1, 0, 0), ((1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1))),
    ((-1, 0, 0), ((0, 1, 0), (0, 0, 0), (0, 0, 1), (0, 1, 1))),
    ((0, 1, 0), ((1, 1, 0), (0, 1, 0), (0, 1, 1), (1, 1, 1))),
    ((0, -1, 0), ((0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1))),
    ((0, 0, 1), ((0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1))),
    ((0, 0, -1), ((0, 1, 0), (1, 1, 0), (1, 0, 0), (0, 0, 0))),
)


def palette_material(colours):
    """A material whose texture holds one colour per pixel, and the pixel of each colour."""
    image = bpy.data.images.new("sheep_palette", PALETTE_SIZE, PALETTE_SIZE, alpha=False)
    pixels = [0.0] * (PALETTE_SIZE * PALETTE_SIZE * 4)
    index = {}
    for n, colour in enumerate(colours):
        pixels[n * 4:n * 4 + 4] = [c / 255 for c in bytes.fromhex(colour[1:])] + [1.0]
        index[colour] = n
    image.pixels.foreach_set(pixels)
    # Saved to a file so that the exporter embeds the pixels rather than a blank generated image
    image.filepath_raw = str(Path(tempfile.mkdtemp()) / "sheep_palette.png")
    image.file_format = "PNG"
    image.save()

    material = bpy.data.materials.new("Sheep")
    nodes = material.node_tree.nodes
    bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Roughness"].default_value = 0.9
    texture = nodes.new("ShaderNodeTexImage")
    texture.image = image
    texture.interpolation = "Closest"
    material.node_tree.links.new(texture.outputs["Color"], bsdf.inputs["Base Color"])
    return material, index


def part_object(name, cells, pivot, hidden_by, material, index, collection):
    """The visible faces of `cells` as one mesh, with its origin on `pivot`."""
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    px, py, pz = pivot
    for (i, j, k), colour in cells.items():
        n = index[colour]
        u, v = ((n % PALETTE_SIZE) + 0.5) / PALETTE_SIZE, ((n // PALETTE_SIZE) + 0.5) / PALETTE_SIZE
        for (dx, dy, dz), corners in FACES:
            neighbour = (i + dx, j + dy, k + dz)
            if neighbour in cells or neighbour in hidden_by:
                continue
            face = bm.faces.new([bm.verts.new(((i + cx - px) * VOX, (j + cy - py) * VOX, (k + cz - pz) * VOX))
                                 for cx, cy, cz in corners])
            for loop in face.loops:
                loop[uv].uv = (u, v)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=VOX / 1000)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(material)
    obj = bpy.data.objects.new(name, mesh)
    obj.location = (px * VOX, py * VOX, pz * VOX)
    collection.objects.link(obj)
    return obj


# ---------------------------------------------------------------------------- the clips

def deg(*angles):
    return tuple(math.radians(a) for a in angles)


def walk(name):
    """One stride: diagonal legs swing together and the body dips as they pass under it."""
    if name.startswith("leg"):
        s = 1 if name in ("leg_FL", "leg_BR") else -1
        return [(f, None, deg(25 * s * a, 0, 0)) for f, a in ((0, 0), (6, 1), (12, 0), (18, -1), (24, 0))]
    if name == "body":
        return [(f, (0, 0, -0.6 * VOX * d), deg(0, 3 * r, 0))
                for f, d, r in ((0, 0, 0), (6, 1, 1), (12, 0, 0), (18, 1, -1), (24, 0, 0))]
    if name == "head":
        return [(f, (0, 0, -0.6 * VOX * d), deg(3 * n, 0, 0))
                for f, d, n in ((0, 0, 1), (6, 1, -1), (12, 0, 1), (18, 1, -1), (24, 0, 1))]
    return [(f, None, deg(0, 0, 18 * a)) for f, a in ((0, 1), (12, -1), (24, 1))]


def eat(name):
    """The head goes down to the grass, chews three times and comes back up."""
    if name == "head":
        down = (0, -1 * VOX, -5 * VOX)
        return [(0, (0, 0, 0), deg(0, 0, 0)), (12, down, deg(55, 0, 0)), (24, down, deg(62, 0, 0)),
                (30, down, deg(50, 0, 0)), (36, down, deg(62, 0, 0)), (42, down, deg(50, 0, 0)),
                (48, down, deg(60, 0, 0)), (60, (0, 0, 0), deg(0, 0, 0)), (72, (0, 0, 0), deg(0, 0, 0))]
    if name == "tail":
        return [(f, None, deg(0, 0, a)) for f, a in ((0, 0), (30, 20), (36, -20), (42, 20), (50, 0), (72, 0))]
    return [(0, (0, 0, 0), deg(0, 0, 0)), (72, (0, 0, 0), deg(0, 0, 0))]


def idle(name):
    """Breathing, a look to one side then the other, a flick of the tail."""
    if name == "body":
        return [(f, (0, 0, 0.2 * VOX * a), None) for f, a in ((0, 0), (18, 1), (36, 0), (54, 1), (72, 0))]
    if name == "head":
        return [(f, None, deg(*r)) for f, r in ((0, (0, 0, 0)), (12, (-6, 4, 14)), (24, (-6, 4, 14)),
                                                (36, (4, -3, -10)), (48, (4, -3, -10)), (60, (0, 0, 0)),
                                                (72, (0, 0, 0)))]
    if name == "tail":
        return [(f, None, deg(0, 0, a)) for f, a in ((0, 0), (12, 25), (24, -25), (36, 0), (72, 0))]
    return [(0, (0, 0, 0), deg(0, 0, 0)), (72, (0, 0, 0), deg(0, 0, 0))]


def add_clip(obj, clip, keys):
    """Keys `obj` from its rest pose, then moves the action to an NLA track named after the clip,
    which the exporter turns into the glTF animation of that name.

    Every clip keys the place and the turn of every part, `None` meaning at rest: a part a clip
    left out would keep, in the game, the pose the previous clip left it in."""
    rest = obj.location.copy()
    obj.animation_data_create()
    obj.animation_data.action = None
    for frame, offset, rotation in keys:
        obj.location = tuple(r + o for r, o in zip(rest, offset or (0, 0, 0)))
        obj.rotation_euler = rotation or (0, 0, 0)
        obj.keyframe_insert("location", frame=frame)
        obj.keyframe_insert("rotation_euler", frame=frame)
    obj.location = rest
    obj.rotation_euler = (0, 0, 0)
    action = obj.animation_data.action
    action.name = f"{obj.name}_{clip}"
    track = obj.animation_data.nla_tracks.new()
    track.name = clip
    track.strips.new(clip, 0, action)
    obj.animation_data.action = None


# ---------------------------------------------------------------------------- the file

def main():
    if bpy.app.background:
        bpy.ops.wm.read_factory_settings(use_empty=True)
        scene = bpy.context.scene
    else:
        scene = bpy.data.scenes.new("SheepGenerated")
        bpy.context.window.scene = scene
    scene.render.fps = FPS

    parts = sheep_cells()
    material, index = palette_material(sorted({c for cells in parts.values() for c in cells.values()}))
    root = bpy.data.objects.new("Sheep", None)
    scene.collection.objects.link(root)
    objects = {}
    for name, pivot in pivots(parts).items():
        hidden_by = parts["body"] if name != "body" else {}
        obj = part_object(name, parts[name], pivot, hidden_by, material, index, scene.collection)
        obj.parent = root
        objects[name] = obj
    for clip, keys in (("Eat", eat), ("Idle", idle), ("Walk", walk)):
        for name, obj in objects.items():
            add_clip(obj, clip, keys(name))

    for obj in scene.objects:
        obj.select_set(obj is root or obj.parent is root)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=str(OUT), export_format="GLB", use_selection=True,
                              use_active_scene=True, export_animations=True,
                              export_animation_mode="NLA_TRACKS",
                              # Keeps the clips that hold a part still, and with them the
                              # tracks in the order they were made: Eat, Idle, Walk
                              export_optimize_animation_keep_anim_object=True)
    print(f"wrote {OUT}: {sum(len(o.data.polygons) for o in objects.values())} faces, "
          f"{len(index)} colours")


main()
