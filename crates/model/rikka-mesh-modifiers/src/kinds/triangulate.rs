use rikka_file_format::{Codec, DecodeError, Reader, Writer};
use rikka_polymesh::{NgonMethod, QuadMethod};

#[derive(Clone, Debug, PartialEq)]
pub struct Triangulate {
    pub quad_method: QuadMethod,
    pub ngon_method: NgonMethod,
    pub min_vertices: u32,
    pub keep_custom_normals: bool,
}

impl Default for Triangulate {
    fn default() -> Triangulate {
        Triangulate {
            quad_method: QuadMethod::ShortEdge,
            ngon_method: NgonMethod::Beauty,
            min_vertices: 4,
            keep_custom_normals: false,
        }
    }
}

impl Codec for Triangulate {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.tag(match self.quad_method {
            QuadMethod::Beauty => 0,
            QuadMethod::Fixed => 1,
            QuadMethod::Alternate => 2,
            QuadMethod::ShortEdge => 3,
            QuadMethod::LongEdge => 4,
        });
        out.tag(match self.ngon_method {
            NgonMethod::Beauty => 0,
            NgonMethod::Clip => 1,
        });
        out.put(&self.min_vertices);
        out.put(&self.keep_custom_normals);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let quad_method = match input.tag()? {
            0 => QuadMethod::Beauty,
            1 => QuadMethod::Fixed,
            2 => QuadMethod::Alternate,
            3 => QuadMethod::ShortEdge,
            4 => QuadMethod::LongEdge,
            _ => return Err(DecodeError::Malformed("unknown quad method")),
        };
        let ngon_method = match input.tag()? {
            0 => NgonMethod::Beauty,
            1 => NgonMethod::Clip,
            _ => return Err(DecodeError::Malformed("unknown n-gon method")),
        };
        let min_vertices: u32 = input.get()?;
        if !(4..=i32::MAX as u32).contains(&min_vertices) {
            return Err(DecodeError::Malformed(
                "triangulate minimum vertices is below 4",
            ));
        }
        Ok(Triangulate {
            quad_method,
            ngon_method,
            min_vertices,
            keep_custom_normals: input.get()?,
        })
    }
}
