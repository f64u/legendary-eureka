use std::{fs::File, io::BufReader, path::Path, vec};

use nalgebra::{Point3, Vector3};
use serde::Deserialize;

use crate::{cell::Cell, disk_util::interlace_alpha, texture_quadtree::TexturedQuadTree};

#[derive(Debug, Deserialize)]
pub struct MapInfo {
    pub name: String,
    #[serde(rename = "h-scale")]
    pub h_scale: f32,
    #[serde(rename = "v-scale")]
    pub v_scale: f32,
    #[serde(rename = "base-elev")]
    pub base_elevation: f32,
    #[serde(rename = "min-elev")]
    pub min_elevation: f32,
    #[serde(rename = "max-elev")]
    pub max_elevation: f32,
    #[serde(rename = "min-sky")]
    pub min_sky: f32,
    #[serde(rename = "max-sky")]
    pub max_sky: f32,
    pub width: u32,
    pub height: u32,
    #[serde(rename = "cell-size")]
    pub cell_width: u32,
    #[serde(rename = "color-map")]
    pub has_color: bool,
    #[serde(rename = "normal-map")]
    pub has_normals: bool,
    #[serde(rename = "water-map")]
    pub has_water: bool,
    #[serde(rename = "sun-dir")]
    pub sun_dir: [f32; 3],
    #[serde(rename = "sun-intensity")]
    pub sun_intensity: [f32; 3],
    #[serde(rename = "ambient")]
    pub ambient_intensity: [f32; 3],
    pub grid: Vec<String>,
    #[serde(rename = "has-fog")]
    pub has_fog: Option<bool>,
    #[serde(rename = "fog-color")]
    pub fog_color: Option<[f32; 3]>,
    #[serde(rename = "fog-density")]
    pub fog_density: Option<f32>,
}

#[derive(Debug)]
pub struct Map {
    pub info: MapInfo,
    pub abstract_size: (usize, usize),
    pub world_size: (f64, f64),
    pub cells: Vec<Vec<Cell>>,
    pub objects: Vec<Vec<()>>,
}

impl Map {
    pub fn new(map_dir: impl AsRef<Path>) -> Result<Self, &'static str> {
        let map_path = map_dir.as_ref().join("map.json");

        let info_file = File::open(map_path).map_err(|_| "Unable to open map.json")?;
        let info_reader = BufReader::new(info_file);
        let info: MapInfo =
            serde_json::from_reader(info_reader).map_err(|_| "Invalid map.json file")?;

        if info.width % info.cell_width != 0 || info.height % info.cell_width != 0 {
            return Err("width and height have to be a multiple of cell_size");
        }

        let abstract_size = (
            (info.width / info.cell_width) as usize,
            (info.height / info.cell_width) as usize,
        );
        let world_size = (info.width as f64, info.height as f64);

        if abstract_size.0 * abstract_size.1 != info.grid.len() {
            return Err("No. of cells does not match the cells in grid");
        }

        let mut cells = Vec::with_capacity(abstract_size.0 as usize);
        for row in 0..abstract_size.0 {
            let mut cell_row = Vec::with_capacity(abstract_size.1 as usize);
            for col in 0..abstract_size.1 {
                let idx = row * abstract_size.1 + col;
                let grid_name = &info.grid[idx as usize];
                let cell_dir = map_dir.as_ref().join(grid_name);
                let color_tqt = if info.has_color {
                    let mut tree = TexturedQuadTree::new(cell_dir.join("color.tqt"))?;
                    for image in tree.lod.mut_view() {
                        interlace_alpha(&mut image.image)
                    }
                    Some(tree)
                } else {
                    None
                };

                let normal_tqt = if info.has_normals {
                    Some(TexturedQuadTree::new(cell_dir.join("norm.tqt"))?)
                } else {
                    None
                };
                cell_row.push(Cell::new(
                    cell_dir.join("hf.cell"),
                    (row as u32, col as u32),
                    color_tqt,
                    normal_tqt,
                    info.cell_width,
                )?);
            }
            cells.push(cell_row);
        }

        let mut map = Map {
            info,
            abstract_size,
            world_size,
            cells,
            objects: vec![],
        };

        let mut cells = Vec::new();

        for cell_row in map.cells.iter() {
            let mut row = Vec::new();
            for cell in cell_row.clone().iter() {
                let mut new_cell = (*cell).clone();
                new_cell.put_in_map(&map);
                row.push(new_cell);
            }
            cells.push(row);
        }

        map.cells = cells;

        Ok(map)
    }

    pub const fn north(&self) -> f64 {
        0.0
    }

    pub fn south(&self) -> f64 {
        self.world_size.1 as f64 * self.info.h_scale as f64
    }

    pub const fn west(&self) -> f64 {
        0.0
    }

    pub fn east(&self) -> f64 {
        self.world_size.0 as f64 * self.info.h_scale as f64
    }

    pub fn scale(&self) -> [f32; 3] {
        let h_scale = self.info.h_scale;
        let v_scale = self.info.v_scale;

        [h_scale, v_scale, h_scale]
    }

    pub fn world_cell_width(&self) -> f64 {
        self.info.cell_width as f64 * self.info.h_scale as f64
    }

    pub fn cell_at_world_pos(&self, (x, z): (f64, f64)) -> Option<&Cell> {
        if x < 0f64 || z < 0f64 {
            return None;
        }

        Some(
            &self.cells[(z / self.world_cell_width()) as usize]
                [(x / self.world_cell_width()) as usize],
        )
    }

    pub fn cell_world_pos(&self, (row, col): (usize, usize)) -> Point3<f64> {
        self.cells[row][col].corner_world_position()
    }
}

#[cfg(test)]
mod test {
    use super::{Map, MapInfo};

    #[test]
    fn can_read_json() {
        let content1 = include_str!("../maps/test-map1/map.json");
        let content2 = include_str!("../maps/test-map2/map.json");
        let d1: MapInfo = serde_json::from_str(content1).unwrap();
        let d2: MapInfo = serde_json::from_str(content2).unwrap();
        println!("{d1:?}\n{d2:?}");
    }

    #[test]
    fn no_map_errors() {
        let m1 = Map::new("maps/test-map1/map.json");
        let m2 = Map::new("maps/test-map2/map.json");
        assert!(m1.is_ok());
        assert!(m2.is_ok());
    }

    #[test]
    fn testing() {
        let m1 = Map::new("maps/test-map2/map.json").unwrap();
        println!("{:?}", m1.cells[0][0].tree.items_at_level(0)[0].chunk)
    }
}
