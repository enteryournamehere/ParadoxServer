use std::collections::HashMap;

use paradox_typed_db::{
    typed_rows::{MissionTaskRow, ObjectsRef},
    typed_tables::MissionTasksTable,
};
use serde::Serialize;

use crate::api::adapter::{FindHash, IdentityHash, TypedTableIterAdapter};

use super::data::MissionTaskUIDLookup;

#[derive(Debug, Clone)]
pub struct MapFilter<'a, E> {
    base: &'a HashMap<i32, E>,
    keys: &'a [i32],
}

pub(super) type ObjectsRefAdapter<'a, 'b> =
    TypedTableIterAdapter<'a, 'b, ObjectsRef<'a, 'b>, IdentityHash, &'b [i32]>;

#[derive(Serialize)]
pub(super) struct ObjectTypeEmbedded<'a, 'b> {
    pub objects: ObjectsRefAdapter<'a, 'b>,
}

impl<'a, E> MapFilter<'a, E> {
    fn to_iter<'b: 'a>(&'b self) -> impl Iterator<Item = (i32, &'a E)> + 'b {
        self.keys
            .iter()
            .filter_map(move |k| self.base.get(k).map(move |v| (*k, v)))
    }
}

impl<'a, E: Serialize> Serialize for MapFilter<'a, E> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.to_iter())
    }
}

pub(super) type MissionTaskHash<'b> = &'b HashMap<i32, MissionTaskUIDLookup>;

pub(super) type MissionTasks<'a, 'b> =
    TypedTableIterAdapter<'a, 'b, MissionTaskRow<'a, 'b>, MissionTaskHash<'b>, &'b [i32]>;

pub(super) struct MissionTaskIconsAdapter<'a, 'b> {
    table: &'b MissionTasksTable<'a>,
    key: i32,
}

impl<'a, 'b> Serialize for MissionTaskIconsAdapter<'a, 'b> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.table.as_task_icon_iter(self.key))
    }
}

#[derive(Clone)]
pub(super) struct MissionsTaskIconsAdapter<'a, 'b> {
    table: &'b MissionTasksTable<'a>,
    keys: &'b [i32],
}

impl<'a, 'b> MissionsTaskIconsAdapter<'a, 'b> {
    pub fn new(table: &'b MissionTasksTable<'a>, keys: &'b [i32]) -> Self {
        Self { table, keys }
    }
}

impl<'a, 'b> Serialize for MissionsTaskIconsAdapter<'a, 'b> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.keys.iter().copied().map(|key| {
            (
                key,
                MissionTaskIconsAdapter {
                    table: self.table,
                    key,
                },
            )
        }))
    }
}

impl<'a> FindHash for HashMap<i32, MissionTaskUIDLookup> {
    fn find_hash(&self, v: i32) -> Option<i32> {
        self.get(&v).map(|r| r.mission)
    }
}
