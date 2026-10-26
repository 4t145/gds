//! Fixed volume pages retain their default mount point. Opening another
//! directory starts an ordinary tab; volume slots stay outside that strip.

use super::*;

#[derive(Default)]
pub(in super::super) struct Pages {
    pub entries: Vec<Page>,
}

pub(in super::super) struct Page {
    pub volume: VolumeId,
    pub slot: OpenTab,
}

impl Pages {
    pub fn ensure(&mut self, volume: &VolumeId, root: Option<&Path>, next_id: &mut u64) -> u64 {
        if let Some(page) = self.entries.iter().find(|page| &page.volume == volume) {
            return page.slot.id;
        }
        let id = *next_id;
        *next_id += 1;
        // An unmounted volume has no path to read. Its unavailable state is
        // installed before the page is displayed; an empty root never loads.
        self.entries.push(Page {
            volume: volume.clone(),
            slot: OpenTab::new(id, root.unwrap_or_else(|| Path::new("")).into(), false),
        });
        id
    }

    pub fn active(&self, id: u64) -> Option<&Page> {
        self.entries.iter().find(|page| page.slot.id == id)
    }
}

/// A saved choice remains explicit even when its mount disappears. Do not
/// silently browse a different subtree after an external remount.
pub(in super::super) fn default_root(
    volume: &storage::Volume,
    preferred: Option<&Path>,
) -> Option<PathBuf> {
    preferred.map(Path::to_owned).or_else(|| {
        volume
            .mount_points
            .iter()
            .min_by(|left, right| {
                left.components()
                    .count()
                    .cmp(&right.components().count())
                    .then_with(|| left.cmp(right))
            })
            .cloned()
    })
}
