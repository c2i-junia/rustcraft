"""Renders a portrait of each creature of the game for the wiki, from its model.

    blender -b -P wiki/scripts/render_creatures.py -- data/models/animated wiki/static/creatures

Writes <Kind>.png, 256x256 on a transparent background, the creature seen from the front left
and a little above, as the inventory shows blocks. Run it again when a model changes.
"""

import math
import os
import sys

import bpy
from mathutils import Vector

args = sys.argv[sys.argv.index("--") + 1 :]
source, target = args[0], args[1]
only = set(args[2:])
os.makedirs(target, exist_ok=True)

# Projectiles and vehicles have no page
SKIP = {"Arrow", "Bobber", "Ember", "ThrownPotion", "Boat"}
GEAR = ("armour_", "saddle", "chest")


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.resolution_x = scene.render.resolution_y = 256
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    engines = [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items]
    scene.render.engine = next(e for e in engines if "EEVEE" in e)
    scene.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("World")
    world.use_nodes = True
    bg = next(n for n in world.node_tree.nodes if n.type == "BACKGROUND")
    bg.inputs[0].default_value = (0.55, 0.5, 0.45, 1)
    bg.inputs[1].default_value = 0.9
    scene.world = world
    return scene


def render(path, name):
    scene = reset()
    bpy.ops.import_scene.gltf(filepath=path)
    # Drawn as found in the wild: without the gear a player puts on it
    for o in list(scene.objects):
        if o.type == "MESH" and o.name.startswith(GEAR):
            bpy.data.objects.remove(o)
    meshes = [o for o in scene.objects if o.type == "MESH"]
    # The first frame of its first animation: its rest pose may be folded
    scene.frame_set(0)
    bpy.context.view_layer.update()
    corners = [
        o.matrix_world @ Vector(c) for o in meshes for c in o.bound_box
    ]
    lo = Vector([min(c[i] for c in corners) for i in range(3)])
    hi = Vector([max(c[i] for c in corners) for i in range(3)])
    centre = (lo + hi) / 2
    size = max(hi - lo)

    cam_data = bpy.data.cameras.new("Camera")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = size * 1.25
    cam = bpy.data.objects.new("Camera", cam_data)
    scene.collection.objects.link(cam)
    # glTF faces +Z forward, Y up, which the importer makes -Y forward, Z up
    direction = Vector((-0.75, -1.0, 0.55)).normalized()
    cam.location = centre + direction * size * 4
    cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()
    cam_data.clip_end = size * 10
    scene.camera = cam

    sun = bpy.data.lights.new("Sun", "SUN")
    sun.energy = 3.0
    sun.angle = math.radians(20)
    light = bpy.data.objects.new("Sun", sun)
    light.rotation_euler = (math.radians(40), math.radians(-10), math.radians(-35))
    scene.collection.objects.link(light)

    scene.render.filepath = os.path.join(target, f"{name}.png")
    bpy.ops.render.render(write_still=True)


for file in sorted(os.listdir(source)):
    name, ext = os.path.splitext(file)
    if ext != ".glb" or name in SKIP or (only and name not in only):
        continue
    render(os.path.join(source, file), name)
    print("rendered", name)
