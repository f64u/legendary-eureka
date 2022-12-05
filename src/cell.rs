use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use nalgebra::Vector3;

use crate::{
    disk_util::read_value,
    map::Map,
    quadtree::{util::full_size, QuadTree},
    texture_quadtree::TexturedQuadTree,
};

struct CellHeader {
    magic: u32,
    compressed: bool,
    size: u32,
    depth: u32,
}

impl CellHeader {
    fn read_from<R: Read>(reader: &mut BufReader<R>) -> Result<Self, &'static str> {
        let mut magic = 0u32;
        let mut compressed = 0u32;
        let mut size = 0u32;
        let mut depth = 0u32;

        read_value(reader, &mut magic, "Unable to read magic no.")?;
        read_value(reader, &mut compressed, "Unable to read compressed flag")?;
        read_value(reader, &mut size, "Unable to read size")?;
        read_value(reader, &mut depth, "Unable to read depth")?;

        Ok(Self {
            magic,
            compressed: compressed != 0,
            size,
            depth,
        })
    }
}

#[derive(Debug)]
pub struct Cell {
    pub position: (u32, u32),
    pub depth: u32,
    pub lod: QuadTree<tile::Tile>,
    pub color_tqt: Option<TexturedQuadTree>,
    pub normal_tqt: Option<TexturedQuadTree>,

    pub worldly_width: Option<f64>,
}

impl Cell {
    const MAGIC: u32 = 0x63656C6C;
    const MIN_DEPTH: u32 = 1;
    const MAX_DEPTH: u32 = 9;

    pub fn new<P: AsRef<Path>>(
        path: P,
        position: (u32, u32),
        color_tqt: Option<TexturedQuadTree>,
        normal_tqt: Option<TexturedQuadTree>,
        cell_width: u32,
    ) -> Result<Self, &'static str> {
        let file = File::open(path).map_err(|_| "Unable to open cell file")?;
        let mut reader = BufReader::new(file);

        let CellHeader {
            magic,
            compressed,
            size,
            depth,
        } = CellHeader::read_from(&mut reader)?;

        if magic != Self::MAGIC {
            return Err("Invalid magic no.");
        }

        if compressed {
            return Err("Compressed cells are not supported yet");
        }

        if size != cell_width {
            return Err("Cell size does not match map cell size");
        }

        if depth < Self::MIN_DEPTH || depth > Self::MAX_DEPTH {
            return Err("Depth out of supported range.");
        }

        let n_tiles = full_size(depth) as usize;
        let mut offsets: Vec<u64> = vec![0; n_tiles];
        for i in 0..n_tiles {
            read_value(&mut reader, &mut offsets[i], "Unable to read offset")?;
        }

        let lod = QuadTree::read_from(&mut reader, depth, &offsets)?;

        Ok(Self {
            position,
            depth,
            lod,
            color_tqt,
            normal_tqt,
            worldly_width: None,
        })
    }

    pub fn is_in_map(&self) -> bool {
        self.worldly_width.is_some()
    }

    pub fn put_in_map(&mut self, map: &Map) {
        self.worldly_width = Some(map.world_cell_width());

        let pos = self.corner_world_position();

        for tile in self.lod.mut_view() {
            tile.put_in_map_in_cell(pos, map)
        }
    }

    pub fn corner_world_position(&self) -> Vector3<f64> {
        match self.worldly_width {
            Some(width) => Vector3::new(
                width * self.position.1 as f64,
                0.0,
                width * self.position.0 as f64,
            ),

            None => panic!("Put the cell in a map first!"),
        }
    }
}

pub mod tile {
    use std::io::{BufReader, Read, Seek};

    use nalgebra::Vector3;

    use crate::{
        aabb::AABB,
        map::Map,
        quadtree::{
            util::{full_size, node_index},
            QuadTree,
        },
    };

    use super::chunk::Chunk;

    #[derive(Debug, Clone)]
    pub struct Tile {
        pub chunk: Chunk,
        pub position: (u32, u32),
        pub level: u32,

        /// Set when put in map
        pub bbox: Option<AABB<f64>>,
    }

    impl Tile {
        pub fn is_in_map(&self) -> bool {
            self.bbox.is_some()
        }

        pub fn put_in_map_in_cell(&mut self, cell_world_pos: Vector3<f64>, map: &Map) {
            let tile_nw_pos = cell_world_pos
                + Vector3::new(
                    map.info.h_scale as f64 * self.position.1 as f64,
                    map.info.base_elevation as f64
                        + map.info.v_scale as f64 * self.chunk.min_y as f64,
                    map.info.h_scale as f64 * self.position.0 as f64,
                );

            let tile_worldly_width =
                map.info.h_scale as f64 * (map.info.cell_width >> self.level) as f64;
            let mut tile_se_pos =
                tile_nw_pos + Vector3::new(tile_worldly_width, 0.0, tile_worldly_width);
            tile_se_pos.y =
                map.info.base_elevation as f64 + map.info.v_scale as f64 * self.chunk.max_y as f64;

            self.bbox = Some(AABB::new(tile_nw_pos, tile_se_pos));
        }
    }

    impl QuadTree<Tile> {
        pub fn read_from<R: Read + Seek>(
            reader: &mut BufReader<R>,
            depth: u32,
            offsets: &[u64],
        ) -> Result<Self, &'static str> {
            let mut tiles = Vec::with_capacity(full_size(depth) as usize);

            for level in 0..depth {
                let n = 2usize.pow(level);

                for row in 0..n as u32 {
                    for col in 0..n as u32 {
                        let chunk = Chunk::read_from(
                            reader,
                            offsets[node_index(level, row, col) as usize],
                        )?;

                        tiles.push(Tile {
                            chunk,
                            position: (row, col),
                            level,
                            bbox: None,
                        });
                    }
                }
            }

            Ok(QuadTree::build_complete_tree(tiles, depth))
        }
    }
}

pub mod chunk {
    use std::io::{BufReader, Read, Seek, SeekFrom};

    use bytemuck::{Pod, Zeroable};
    use vulkano::impl_vertex;

    use crate::disk_util::read_value;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
    pub struct HFVertex {
        pub position: [f32; 3],
        pub morph_delta: f32,
    }

    impl_vertex!(HFVertex, position, morph_delta);

    impl HFVertex {
        fn read_from<R: Read>(reader: &mut BufReader<R>) -> Result<Self, &'static str> {
            let mut x = 0i16;
            let mut y = 0i16;
            let mut z = 0i16;
            let mut morph_delta = 0i16;

            read_value(reader, &mut x, "Unable to read vertex x")?;
            read_value(reader, &mut y, "Unable to read vertex y")?;
            read_value(reader, &mut z, "Unable to read vertex z")?;
            read_value(
                reader,
                &mut morph_delta,
                "Unable to read vertex morph delta",
            )?;

            Ok(Self {
                position: [x as f32, y as f32, z as f32],
                morph_delta: morph_delta as f32,
            })
        }
    }

    struct ChunkHeader {
        max_error: f32,
        n_verts: u32,
        n_indices: u32,
        min_y: i16,
        max_y: i16,
    }

    impl ChunkHeader {
        fn read_from<R: Read>(reader: &mut BufReader<R>) -> Result<Self, &'static str> {
            let mut max_error = 0f32;
            let mut n_verts = 0u32;
            let mut n_indices = 0u32;
            let mut min_y = 0i16;
            let mut max_y = 0i16;

            read_value(reader, &mut max_error, "Unable to read chunk max error")?;
            read_value(reader, &mut n_verts, "Unable to read chunk no. of vertices")?;
            read_value(
                reader,
                &mut n_indices,
                "Unable to read chunk no. of indices",
            )?;
            read_value(reader, &mut min_y, "Unable to read chunk minimum y")?;
            read_value(reader, &mut max_y, "Unable to read chunk maximum y")?;

            Ok(Self {
                max_error,
                n_verts,
                n_indices,
                min_y,
                max_y,
            })
        }
    }

    #[derive(Debug, Clone)]
    pub struct Chunk {
        pub max_error: f32,
        pub min_y: i16,
        pub max_y: i16,
        pub vertices: Vec<HFVertex>,
        pub indices: Vec<u16>,
    }

    impl Chunk {
        pub fn read_from<R: Read + Seek>(
            reader: &mut BufReader<R>,
            offset: u64,
        ) -> Result<Self, &'static str> {
            reader
                .seek(SeekFrom::Start(offset))
                .map_err(|_| "Unable to seek to chunk")?;

            let ChunkHeader {
                max_error,
                n_verts,
                n_indices,
                min_y,
                max_y,
            } = ChunkHeader::read_from(reader)?;

            let mut vertices = Vec::with_capacity(n_verts as usize);
            for _ in 0..n_verts {
                vertices.push(HFVertex::read_from(reader)?);
            }

            let mut indices = Vec::with_capacity(n_indices as usize);
            for _ in 0..n_indices {
                let mut x = 0u16;
                read_value(reader, &mut x, "Unable to read index")?;
                indices.push(x);
            }

            Ok(Self {
                max_error,
                min_y,
                max_y,
                vertices,
                indices,
            })
        }
    }
}
