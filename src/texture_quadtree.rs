use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use crate::quadtree::{util::full_size, QuadTree};
use crate::{disk_util::read_value, quadtree::util::node_index};

#[derive(Debug, Clone)]
pub struct Tile {
    pub image: Vec<u8>,
}

impl Tile {
    fn read_from<R: Seek + Read>(
        reader: &mut BufReader<R>,
        tile_size: u32,
        offset: u64,
    ) -> Result<Self, &'static str> {
        reader
            .seek(SeekFrom::Start(offset))
            .map_err(|_| "Unable to seek to tile")?;
        let decoder = png::Decoder::new(reader);
        let mut png_reader = decoder.read_info().map_err(|_| "Unable to read png")?;

        let mut image = vec![0; png_reader.output_buffer_size()];
        let r = png_reader
            .next_frame(image.as_mut_slice())
            .map_err(|_| "Unable to read tile")?;

        if r.width != tile_size || r.height != tile_size {
            return Err("Invalid tile size??");
        }

        Ok(Self { image })
    }
}

#[derive(Debug)]
struct Header {
    magic: u32,
    version: u32,
    depth: u32,
    tile_size: u32,
}

impl Header {
    fn read_from<R: Read>(reader: &mut BufReader<R>) -> Result<Self, &'static str> {
        let mut magic: u32 = 0;
        let mut version: u32 = 0;
        let mut depth: u32 = 0;
        let mut tile_size: u32 = 0;

        read_value(reader, &mut magic, "Unable to read magic no.".into())?;
        read_value(reader, &mut version, "Unable to read version no.".into())?;
        read_value(reader, &mut depth, "Unable to read depth")?;
        read_value(reader, &mut tile_size, "Unable to read tile size".into())?;

        Ok(Self {
            magic,
            version,
            depth,
            tile_size,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TexturedQuadTree {
    pub lod: QuadTree<Tile>,
    pub depth: u32,
    pub tile_size: u32,
}

impl QuadTree<Tile> {
    fn read_from<R: Read + Seek>(
        reader: &mut BufReader<R>,
        depth: u32,
        tile_size: u32,
        offsets: &[u64],
    ) -> Result<Self, &'static str> {
        let mut tiles = Vec::with_capacity(full_size(depth) as usize);

        for level in 0..depth {
            let n = 2usize.pow(level);
            for row in 0..n as u32 {
                for col in 0..n as u32 {
                    tiles.push(Tile::read_from(
                        reader,
                        tile_size,
                        offsets[node_index(level, row, col) as usize],
                    )?)
                }
            }
        }

        Ok(QuadTree::build_complete_tree(tiles, depth))
    }
}

impl TexturedQuadTree {
    const MAGIC: u32 = 0x00545154;
    const VERSION: u32 = 1;

    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, &'static str> {
        let file = File::open(path).map_err(|_| "Error while opening texture file")?;
        let mut reader = BufReader::new(file);

        let Header {
            magic,
            version,
            depth,
            tile_size,
        } = Header::read_from(&mut reader)?;

        if magic != Self::MAGIC {
            return Err("Invalid magic no.");
        }

        if version != Self::VERSION {
            return Err("Invalid version no.");
        }

        let n_tiles = full_size(depth) as usize;
        let mut offsets: Vec<u64> = vec![0; n_tiles];

        for i in 0..n_tiles {
            read_value(&mut reader, &mut offsets[i], "Unable to read offset")?;
        }

        let lod = QuadTree::<Tile>::read_from(&mut reader, depth, tile_size, &offsets)?;

        Ok(Self {
            lod,
            depth,
            tile_size,
        })
    }
}

#[cfg(test)]
mod test {
    use super::TexturedQuadTree;

    #[test]
    fn can_read_file() {
        for tqt in vec![
            TexturedQuadTree::new("maps/test-map1/00_00/color.tqt").unwrap(),
            TexturedQuadTree::new("maps/test-map1/00_00/norm.tqt").unwrap(),
            TexturedQuadTree::new("maps/test-map2/00_00/color.tqt").unwrap(),
            TexturedQuadTree::new("maps/test-map2/00_00/norm.tqt").unwrap(),
        ] {
            let n = tqt.lod.depth();
            println!("{n:?}")
        }
    }
}
