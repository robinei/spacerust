use crate::CursorPosition;
use bevy::prelude::*;
use std::{collections::{hash_map::Entry, HashMap}, f32::consts::PI};

const CELL_DIM: f32 = 20.0;

pub type Cell = (i32, i32);

fn calc_cell(pos: Vec3) -> Cell {
    ((pos.x / CELL_DIM).floor() as i32, (pos.z / CELL_DIM).floor() as i32)
}

#[derive(Resource)]
pub struct SpatialIndex {
    cells: HashMap<Cell, Vec<Entity>>,
}

#[derive(Component)]
pub struct CellAssociation {
    cell: Cell,
    new_cell: Cell,
}

impl CellAssociation {
    pub fn new() -> Self {
        Self {
            cell: (i32::MIN, i32::MIN),
            new_cell: (i32::MIN, i32::MIN),
        }
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HasDirtyCell;

impl SpatialIndex {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new()
        }
    }

    pub fn query<F: FnMut(Entity)>(&self, pos: Vec3, radius: f32, mut handler: F) {
        self.query_cells(pos, radius, |cell| {
            if let Some(vec) = self.cells.get(&cell) {
                for entity in vec {
                    handler(*entity);
                }
            }
        });
    }

    pub fn query_cells<F: FnMut(Cell)>(&self, pos: Vec3, radius: f32, mut handler: F) {
        let cx = pos.x / CELL_DIM;
        let cz = pos.z / CELL_DIM;
        let r = radius / CELL_DIM;
        let minz = (cz - r).floor() as i32;
        let maxz = ((cz + r).ceil() as i32) - 1;

        for z in minz..=maxz {
            let ztest = if ((z + 1) as f32) < cz {
                (z + 1) as f32
            } else if (z as f32) > cz {
                z as f32
            } else {
                cz
            };

            let zdist2 = (ztest - cz)*(ztest - cz);
            let xdiff = (r*r - zdist2).sqrt();
            let minx = (cx - xdiff).floor() as i32;
            let maxx = ((cx + xdiff).ceil() as i32) - 1;

            for x in minx..=maxx {
                handler((x, z));
            }
        }
    }

    fn insert(&mut self, cell: Cell, entity: Entity) {
        self.cells.entry(cell).or_insert_with(|| Vec::new()).push(entity);
        if cell == (0, 0) {
            println!("inserted. new size: {}", self.cells.get(&cell).unwrap().len());
        }
    }

    fn remove(&mut self, cell: Cell, entity: Entity) {
        if let Entry::Occupied(mut occupied) = self.cells.entry(cell) {
            let vec = occupied.get_mut();
            vec.retain_mut(|e| *e != entity);
            if cell == (0, 0) {
                println!("removed. new size: {}", vec.len());
            }
            if vec.len() == 0 {
                occupied.remove_entry();
            }
        }
    }
}

pub fn update_cell_association(
    mut commands: Commands,
    mut query: Query<(Entity, &Transform, &mut CellAssociation), Without<HasDirtyCell>>,
) {
    for (entity, transform, mut cell_assoc) in &mut query {
        cell_assoc.new_cell = calc_cell(transform.translation);
        if cell_assoc.new_cell != cell_assoc.cell {
            commands.entity(entity).insert(HasDirtyCell);
        }
    }
}

pub fn update_spatial_index(
    mut commands: Commands,
    mut query: Query<(Entity, &mut CellAssociation), With<HasDirtyCell>>,
    mut index: ResMut<SpatialIndex>
) {
    for (entity, mut cell_assoc) in &mut query {
        index.remove(cell_assoc.cell, entity);
        index.insert(cell_assoc.new_cell, entity);
        cell_assoc.cell = cell_assoc.new_cell;
        commands.entity(entity).remove::<HasDirtyCell>();
    }
}

pub fn test_spatial_index(
    transforms: Query<&Transform>,
    cursor: Res<CursorPosition>,
    index: Res<SpatialIndex>,
    mut gizmos: Gizmos
) {
    let radius = 5.0;
    gizmos.circle(cursor.position, Direction3d::Y, radius, Color::RED);

    index.query_cells(cursor.position, radius, |cell| {
        gizmos.rect(Vec3::new((cell.0 as f32)*CELL_DIM+CELL_DIM*0.5, 0.0, (cell.1 as f32)*CELL_DIM+CELL_DIM*0.5), Quat::from_axis_angle(Vec3::X, PI*0.5), Vec2::new(CELL_DIM, CELL_DIM), Color::WHITE);
    });

    index.query(cursor.position, radius, |entity| {
        if let Ok(transform) = transforms.get(entity) {
            if (transform.translation - cursor.position).length() < radius {
                gizmos.circle(transform.translation, Direction3d::Y, 1.0, Color::RED);
            }
        }
    });
}
