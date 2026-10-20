//! What the catalog lists: its order, the search and the texts of the panel.

use shared::world::{ItemCategory, ItemId};

use crate::ui::hud::item_name;

/// Under the message saying that nothing matches.
#[allow(dead_code)] // first used by the item browser panel (Task 4)
pub const NO_MATCH_HINT: &str = "Esc clears the search.";

/// Every item, by category in the order of `ItemCategory::ALL`, then in the order of `ItemId::ALL`.
#[allow(dead_code)] // first used by the item browser panel (Task 4)
pub fn catalog() -> Vec<ItemId> {
    ItemCategory::ALL
        .into_iter()
        .flat_map(|category| {
            ItemId::ALL
                .into_iter()
                .filter(move |item| item.category() == category)
        })
        .collect()
}

/// Whether `item` answers the search `query`, whose every word must match, whatever the case. A
/// word starting with `#` matches when the name of the item's category starts with the rest; any
/// other word matches when the name of the item holds it, which covers a word of the name starting
/// with it.
pub fn matches(query: &str, item: ItemId) -> bool {
    let name = item_name(item).to_lowercase();
    let category = item.category().name().to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| match word.strip_prefix('#') {
            Some(prefix) => category.starts_with(prefix),
            None => name.contains(word),
        })
}

/// Whether the catalog shows `item` for the search `query` in `category`, `None` being All.
#[allow(dead_code)] // first used by the item browser panel (Task 4)
pub fn shown_in(query: &str, category: Option<ItemCategory>, item: ItemId) -> bool {
    category.is_none_or(|category| item.category() == category) && matches(query, item)
}

/// How many items the catalog shows, out of all of them.
#[allow(dead_code)] // first used by the item browser panel (Task 4)
pub fn count_label(shown: usize, total: usize) -> String {
    format!("{shown} / {total}")
}

/// What the catalog says when nothing answers `query` in `category`.
#[allow(dead_code)] // first used by the item browser panel (Task 4)
pub fn no_match_message(query: &str, category: Option<ItemCategory>) -> String {
    match category {
        Some(category) => format!("No item matches \"{query}\" in {}.", category.name()),
        None => format!("No item matches \"{query}\"."),
    }
}

/// The second line of the tooltip of an item: its category, and whether it is a tool.
#[allow(dead_code)] // first used by the item browser panel (Task 4)
pub fn category_detail(item: ItemId) -> String {
    let category = item.category().name();
    if item.tool().is_some() {
        format!("{category} · Tool")
    } else {
        category.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ItemCategory::*;

    /// The items of the catalog that `query` and `category` keep.
    fn found(query: &str, category: Option<ItemCategory>) -> Vec<ItemId> {
        catalog()
            .into_iter()
            .filter(|&item| shown_in(query, category, item))
            .collect()
    }

    #[test]
    fn the_catalog_lists_every_item_once_by_category() {
        let items = catalog();
        assert_eq!(items.len(), ItemId::ALL.len());
        for item in ItemId::ALL {
            assert_eq!(items.iter().filter(|&&i| i == item).count(), 1, "{item:?}");
        }
        let rank = |item: &ItemId| {
            ItemCategory::ALL
                .iter()
                .position(|&c| c == item.category())
                .unwrap()
        };
        assert!(items.is_sorted_by_key(rank));
        assert_eq!(items.first(), Some(&ItemId::Dirt));
        assert_eq!(items.last(), Some(&ItemId::WoodenSword));
        assert_eq!(
            found("", Some(Wood)),
            [
                ItemId::OakLog,
                ItemId::OakPlanks,
                ItemId::SpruceLog,
                ItemId::SprucePlanks
            ]
        );
    }

    #[test]
    fn each_word_of_the_search_must_match_the_name() {
        assert_eq!(found("sp pl", None), [ItemId::SprucePlanks]);
        assert_eq!(
            found("oak", None),
            [ItemId::OakLog, ItemId::OakPlanks, ItemId::OakLeaves]
        );
        assert_eq!(
            found("PLANKS", None),
            [ItemId::OakPlanks, ItemId::SprucePlanks]
        );
        assert_eq!(
            found("ore", None),
            [ItemId::CoalOre, ItemId::IronOre, ItemId::GoldOre]
        );
        assert_eq!(found("", None).len(), 33);
        assert_eq!(found("  ", None).len(), 33);
        assert!(found("zzz", None).is_empty());
    }

    #[test]
    fn a_hash_word_matches_the_category() {
        assert_eq!(found("#tools", None).len(), 5);
        assert!(found("#tools", None)
            .iter()
            .all(|item| item.category() == Tools));
        assert_eq!(found("#wo", None), found("", Some(Wood)));
        assert_eq!(found("#", None).len(), 33);
        assert_eq!(found("#ores coal", None), [ItemId::CoalOre]);
        assert!(found("#tools planks", None).is_empty());
    }

    #[test]
    fn a_category_keeps_only_its_items() {
        assert!(shown_in("", Some(Wood), ItemId::OakLog));
        assert!(!shown_in("", Some(Wood), ItemId::Dirt));
        assert!(shown_in("oak", None, ItemId::OakLeaves));
        assert!(!shown_in("oak", Some(Wood), ItemId::OakLeaves));
    }

    #[test]
    fn the_texts_of_the_panel() {
        assert_eq!(count_label(12, 33), "12 / 33");
        assert_eq!(no_match_message("zzz", None), "No item matches \"zzz\".");
        assert_eq!(
            no_match_message("zzz", Some(Wood)),
            "No item matches \"zzz\" in Wood."
        );
        assert_eq!(NO_MATCH_HINT, "Esc clears the search.");
        assert_eq!(category_detail(ItemId::OakLog), "Wood");
        assert_eq!(category_detail(ItemId::WoodenPickaxe), "Tools · Tool");
    }
}
