use std::collections::{BTreeMap, BTreeSet, HashMap};

use assembly_data::fdb::common::Latin1Str;
use paradox_typed_db::TypedDatabase;
use serde::Serialize;

use crate::data::skill_system::match_action_key;

#[derive(Default, Debug, Clone, Serialize)]
pub struct SkillIdLookup {
    /// This field collects all the `uid`s of mission tasks that use this skill
    ///
    pub mission_tasks: Vec<i32>,
    /// The objects that can cast this skill
    pub objects: Vec<i32>,
    /// The item sets that enable this skill
    pub item_sets: Vec<i32>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct BehaviorKeyIndex {
    skill: BTreeSet<i32>,
    uses: BTreeSet<i32>,
    used_by: BTreeSet<i32>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ComponentUse {
    pub lots: Vec<i32>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ComponentsUse {
    /// Map from component_id to list of object_id
    pub components: BTreeMap<i32, ComponentUse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MissionTaskUIDLookup {
    pub mission: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReverseLookup {
    pub mission_task_uids: HashMap<i32, MissionTaskUIDLookup>,
    pub skill_ids: HashMap<i32, SkillIdLookup>,
    pub behaviors: BTreeMap<i32, BehaviorKeyIndex>,
    pub mission_types: BTreeMap<String, BTreeMap<String, Vec<i32>>>,

    pub object_types: BTreeMap<String, Vec<i32>>,
    pub component_use: BTreeMap<i32, ComponentsUse>,
}

impl ReverseLookup {
    pub(crate) fn new(db: &'_ TypedDatabase<'_>) -> Self {
        let mut skill_ids: HashMap<i32, SkillIdLookup> = HashMap::new();
        let mut mission_task_uids = HashMap::new();
        let mut mission_types: BTreeMap<String, BTreeMap<String, Vec<i32>>> = BTreeMap::new();

        for m in db.missions.row_iter() {
            let id = m.id();
            let d_type = m
                .defined_type()
                .map(Latin1Str::decode)
                .unwrap_or_default()
                .into_owned();
            let d_subtype = m
                .defined_subtype()
                .map(Latin1Str::decode)
                .unwrap_or_default()
                .into_owned();
            mission_types
                .entry(d_type)
                .or_default()
                .entry(d_subtype)
                .or_default()
                .push(id)
        }

        for r in db.mission_tasks.row_iter() {
            let uid = r.uid();
            let id = r.id();
            mission_task_uids.insert(uid, MissionTaskUIDLookup { mission: id });

            if r.task_type() == 10 {
                if let Some(p) = r.task_param1() {
                    for num in p.decode().split(',').map(str::parse).filter_map(Result::ok) {
                        skill_ids.entry(num).or_default().mission_tasks.push(uid);
                    }
                }
            }
            //skill_ids.entry(r.uid()).or_default().mission_tasks.push(r
        }
        for s in db.object_skills.row_iter() {
            skill_ids
                .entry(s.skill_id())
                .or_default()
                .objects
                .push(s.object_template());
        }
        for s in db.item_set_skills.row_iter() {
            skill_ids
                .entry(s.skill_id())
                .or_default()
                .item_sets
                .push(s.skill_set_id());
        }

        let mut object_types = BTreeMap::<_, Vec<_>>::new();
        for o in db.objects.row_iter() {
            let id = o.id();
            let ty = o.r#type().decode().into_owned();

            let entry = object_types.entry(ty).or_default();
            entry.push(id);
        }

        let mut component_use: BTreeMap<i32, ComponentsUse> = BTreeMap::new();
        for creg in db.comp_reg.row_iter() {
            let id = creg.id();
            let ty = creg.component_type();
            let cid = creg.component_id();
            let ty_entry = component_use.entry(ty).or_default();
            let co_entry = ty_entry.components.entry(cid).or_default();
            co_entry.lots.push(id);
        }

        let mut behaviors: BTreeMap<i32, BehaviorKeyIndex> = BTreeMap::new();
        for bp in db.behavior_parameters.row_iter() {
            let parameter_id = bp.parameter_id();
            let behavior_id = bp.behavior_id();
            if match_action_key(parameter_id) {
                let value = bp.value() as i32;
                behaviors.entry(behavior_id).or_default().uses.insert(value);
                behaviors
                    .entry(value)
                    .or_default()
                    .used_by
                    .insert(behavior_id);
            }
        }

        for skill in db.skills.row_iter() {
            let bid = skill.behavior_id();
            let skid = skill.skill_id();
            behaviors.entry(bid).or_default().skill.insert(skid);
        }

        Self {
            behaviors,
            skill_ids,
            mission_task_uids,
            mission_types,

            object_types,
            component_use,
        }
    }

    pub(crate) fn get_behavior_set(&self, root: i32) -> BTreeSet<i32> {
        let mut todo = Vec::new();
        let mut all = BTreeSet::new();
        todo.push(root);

        while let Some(next) = todo.pop() {
            if !all.contains(&next) {
                all.insert(next);
                if let Some(data) = self.behaviors.get(&next) {
                    todo.extend(data.uses.iter().filter(|&&x| x > 0));
                }
            }
        }
        all
    }
}
