use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use rikka_model::vrm::{
    AvatarPermission, CommercialUsage, CreditNotation, LegacyLicense, LegacyUser, LicenseName,
    Meta, Modification, VrmVersion,
};
use serde::Deserialize;

use crate::details::{Clip, Details, Embedded, ModelDetails, MotionDetails, VrmDetails};
use crate::format::Format;

use super::fault::{Fault, Probed, clean, read_limited};

const GLB_MAGIC: &[u8; 4] = b"glTF";
const JSON_CHUNK: &[u8; 4] = b"JSON";
const BIN_CHUNK: &[u8; 4] = b"BIN\0";
const GLB_HEADER: u64 = 12;
const CHUNK_HEADER: u64 = 8;
const LARGEST_JSON: u64 = 128 << 20;
const LARGEST_PICTURE: u64 = 32 << 20;
const VRM0_LICENSE_URL: &str = "https://vrm.dev/licenses/1.0/";

pub(super) fn read(path: &Path, format: Format) -> Probed<Details> {
    let container = Container::read(path)?;
    let document: Document =
        serde_json::from_slice(&container.json).map_err(|_| Fault::Malformed("glTF document"))?;
    let clips = document.clips();
    if format == Format::Vrma {
        if document.extensions.vrm_animation.is_none() {
            return Err(Fault::Foreign("VRM animation"));
        }
        return Ok(Details::Motion(Box::new(MotionDetails {
            seconds: clips.iter().map(|clip| clip.seconds).fold(0.0, f32::max),
            body: !clips.is_empty(),
            clips,
            ..MotionDetails::default()
        })));
    }
    let vrm = document.vrm();
    if format == Format::Vrm && vrm.is_none() {
        return Err(Fault::Foreign("VRM"));
    }
    let embedded = vrm
        .as_ref()
        .and_then(|(_, picture)| *picture)
        .and_then(|image| document.embedded(image, container.binary));
    let vrm = vrm.map(|(details, _)| details);
    Ok(Details::Model(Box::new(ModelDetails {
        title: vrm
            .as_ref()
            .and_then(|vrm| clean(&vrm.meta.name))
            .or_else(|| document.scene_name()),
        author: vrm
            .as_ref()
            .and_then(|vrm| clean(&vrm.meta.authors.join(", ")))
            .or_else(|| document.asset.copyright.as_deref().and_then(clean)),
        comment: None,
        vertices: Some(document.vertices()),
        materials: u32::try_from(document.materials.len()).ok(),
        meshes: u32::try_from(document.meshes.len()).ok(),
        clips,
        vrm,
        embedded,
    })))
}

pub(crate) fn embedded_range(path: &Path, offset: u64, length: u64) -> Probed<Vec<u8>> {
    if length > LARGEST_PICTURE {
        return Err(Fault::TooLarge);
    }
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; usize::try_from(length).map_err(|_| Fault::TooLarge)?];
    file.read_exact(&mut bytes).map_err(Fault::read)?;
    Ok(bytes)
}

struct Container {
    json: Vec<u8>,
    binary: Option<Binary>,
}

#[derive(Clone, Copy)]
struct Binary {
    offset: u64,
    length: u64,
}

impl Container {
    fn read(path: &Path) -> Probed<Container> {
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();
        let mut magic = [0; 4];
        let glb = match file.read_exact(&mut magic) {
            Ok(()) => &magic == GLB_MAGIC,
            Err(_) => false,
        };
        if !glb {
            return Ok(Container {
                json: read_limited(path, LARGEST_JSON)?,
                binary: None,
            });
        }
        let mut header = [0; 8];
        file.read_exact(&mut header).map_err(Fault::read)?;
        let (json_length, kind) = chunk(&mut file)?;
        if &kind != JSON_CHUNK {
            return Err(Fault::Malformed("GLB container"));
        }
        if json_length > LARGEST_JSON {
            return Err(Fault::TooLarge);
        }
        if GLB_HEADER + CHUNK_HEADER + json_length > size {
            return Err(Fault::Truncated);
        }
        let mut json = vec![0; usize::try_from(json_length).map_err(|_| Fault::TooLarge)?];
        file.read_exact(&mut json).map_err(Fault::read)?;
        let binary_header = GLB_HEADER + CHUNK_HEADER + json_length.next_multiple_of(4);
        let binary = match binary_header + CHUNK_HEADER <= size {
            false => None,
            true => {
                file.seek(SeekFrom::Start(binary_header))?;
                let (length, kind) = chunk(&mut file)?;
                let offset = binary_header + CHUNK_HEADER;
                (&kind == BIN_CHUNK && offset + length <= size).then_some(Binary { offset, length })
            }
        };
        Ok(Container { json, binary })
    }
}

fn chunk(file: &mut File) -> Probed<(u64, [u8; 4])> {
    let mut header = [0; 8];
    file.read_exact(&mut header).map_err(Fault::read)?;
    let length = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    Ok((
        u64::from(length),
        [header[4], header[5], header[6], header[7]],
    ))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Document {
    asset: Asset,
    scenes: Vec<Scene>,
    meshes: Vec<Mesh>,
    materials: Vec<serde::de::IgnoredAny>,
    accessors: Vec<Accessor>,
    animations: Vec<Animation>,
    images: Vec<Image>,
    textures: Vec<Texture>,
    #[serde(rename = "bufferViews")]
    buffer_views: Vec<BufferView>,
    extensions: Extensions,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Asset {
    copyright: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Scene {
    name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Mesh {
    primitives: Vec<Primitive>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Primitive {
    attributes: Attributes,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Attributes {
    #[serde(rename = "POSITION")]
    position: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Accessor {
    count: u64,
    max: Vec<f64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Animation {
    name: Option<String>,
    samplers: Vec<Sampler>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Sampler {
    input: usize,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Image {
    #[serde(rename = "bufferView")]
    buffer_view: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Texture {
    source: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BufferView {
    buffer: usize,
    #[serde(rename = "byteOffset")]
    byte_offset: u64,
    #[serde(rename = "byteLength")]
    byte_length: u64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Extensions {
    #[serde(rename = "VRM")]
    vrm0: Option<Vrm0>,
    #[serde(rename = "VRMC_vrm")]
    vrm1: Option<Vrm1>,
    #[serde(rename = "VRMC_vrm_animation")]
    vrm_animation: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Vrm0 {
    meta: Option<Vrm0Meta>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Vrm0Meta {
    title: Option<String>,
    version: Option<String>,
    author: Option<String>,
    contact_information: Option<String>,
    reference: Option<String>,
    texture: Option<i64>,
    allowed_user_name: Option<String>,
    violent_ussage_name: Option<String>,
    sexual_ussage_name: Option<String>,
    commercial_ussage_name: Option<String>,
    other_permission_url: Option<String>,
    license_name: Option<String>,
    other_license_url: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Vrm1 {
    meta: Option<Vrm1Meta>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Vrm1Meta {
    name: Option<String>,
    version: Option<String>,
    authors: Vec<String>,
    copyright_information: Option<String>,
    contact_information: Option<String>,
    references: Vec<String>,
    third_party_licenses: Option<String>,
    thumbnail_image: Option<usize>,
    license_url: Option<String>,
    avatar_permission: Option<String>,
    allow_excessively_violent_usage: bool,
    allow_excessively_sexual_usage: bool,
    commercial_usage: Option<String>,
    allow_political_or_religious_usage: bool,
    allow_antisocial_or_hate_usage: bool,
    credit_notation: Option<String>,
    allow_redistribution: bool,
    modification: Option<String>,
    other_license_url: Option<String>,
}

impl Document {
    fn clips(&self) -> Vec<Clip> {
        self.animations
            .iter()
            .enumerate()
            .map(|(index, animation)| Clip {
                name: animation
                    .name
                    .as_deref()
                    .and_then(clean)
                    .unwrap_or_else(|| format!("{}", index + 1)),
                seconds: animation
                    .samplers
                    .iter()
                    .filter_map(|sampler| self.accessors.get(sampler.input)?.max.first().copied())
                    .fold(0.0, f64::max) as f32,
            })
            .collect()
    }

    fn vertices(&self) -> u64 {
        self.meshes
            .iter()
            .flat_map(|mesh| &mesh.primitives)
            .filter_map(|primitive| self.accessors.get(primitive.attributes.position?))
            .map(|accessor| accessor.count)
            .fold(0, u64::saturating_add)
    }

    fn scene_name(&self) -> Option<String> {
        self.scenes
            .iter()
            .find_map(|scene| scene.name.as_deref().and_then(clean))
    }

    fn vrm(&self) -> Option<(VrmDetails, Option<usize>)> {
        if let Some(meta) = self
            .extensions
            .vrm1
            .as_ref()
            .and_then(|vrm| vrm.meta.as_ref())
        {
            return Some((vrm1(meta), meta.thumbnail_image));
        }
        let vrm0 = self.extensions.vrm0.as_ref()?;
        let described = vrm0.meta.as_ref();
        let picture = described
            .and_then(|meta| meta.texture)
            .and_then(|texture| usize::try_from(texture).ok())
            .and_then(|texture| self.textures.get(texture)?.source);
        Some((vrm0_details(described), picture))
    }

    fn embedded(&self, image: usize, binary: Option<Binary>) -> Option<Embedded> {
        let binary = binary?;
        let view = self
            .buffer_views
            .get(self.images.get(image)?.buffer_view?)?;
        let end = view.byte_offset.checked_add(view.byte_length)?;
        (view.buffer == 0 && end <= binary.length && view.byte_length > 0).then_some(
            Embedded::Range {
                offset: binary.offset + view.byte_offset,
                length: view.byte_length,
            },
        )
    }
}

fn vrm1(described: &Vrm1Meta) -> VrmDetails {
    let named = |name: &Option<String>| name.as_deref().and_then(clean);
    let meta = Meta {
        name: named(&described.name).unwrap_or_default(),
        version: named(&described.version),
        authors: described
            .authors
            .iter()
            .filter_map(|author| clean(author))
            .collect(),
        copyright: named(&described.copyright_information),
        contact: named(&described.contact_information),
        references: described
            .references
            .iter()
            .filter_map(|reference| clean(reference))
            .collect(),
        third_party_licenses: named(&described.third_party_licenses),
        thumbnail: None,
        license_url: named(&described.license_url).unwrap_or_default(),
        other_license_url: named(&described.other_license_url),
        avatar_permission: described
            .avatar_permission
            .as_deref()
            .and_then(AvatarPermission::from_name)
            .unwrap_or_default(),
        commercial_usage: described
            .commercial_usage
            .as_deref()
            .and_then(CommercialUsage::from_name)
            .unwrap_or_default(),
        credit_notation: described
            .credit_notation
            .as_deref()
            .and_then(CreditNotation::from_name)
            .unwrap_or_default(),
        modification: described
            .modification
            .as_deref()
            .and_then(Modification::from_name)
            .unwrap_or_default(),
        allow_redistribution: described.allow_redistribution,
        allow_excessively_violent_usage: described.allow_excessively_violent_usage,
        allow_excessively_sexual_usage: described.allow_excessively_sexual_usage,
        allow_political_or_religious_usage: described.allow_political_or_religious_usage,
        allow_antisocial_or_hate_usage: described.allow_antisocial_or_hate_usage,
    };
    VrmDetails {
        version: VrmVersion::V1,
        meta,
        legacy: None,
    }
}

fn vrm0_details(described: Option<&Vrm0Meta>) -> VrmDetails {
    let allowed = |name: &Option<String>| name.as_deref() == Some("Allow");
    let named = |name: &Option<String>| name.as_deref().and_then(clean);
    let license = described.map_or_else(LegacyLicense::default, |described| LegacyLicense {
        allowed_user: described
            .allowed_user_name
            .as_deref()
            .and_then(LegacyUser::from_name)
            .unwrap_or_default(),
        violent_usage: allowed(&described.violent_ussage_name),
        sexual_usage: allowed(&described.sexual_ussage_name),
        commercial_usage: allowed(&described.commercial_ussage_name),
        other_permission_url: named(&described.other_permission_url),
        license: described
            .license_name
            .as_deref()
            .and_then(LicenseName::from_name)
            .unwrap_or(LicenseName::Other),
        other_license_url: named(&described.other_license_url),
    });
    let mut meta = Meta {
        license_url: VRM0_LICENSE_URL.to_owned(),
        ..Meta::default()
    };
    license.apply_to(&mut meta);
    if let Some(described) = described {
        meta.name = named(&described.title).unwrap_or_default();
        meta.version = named(&described.version);
        meta.authors = named(&described.author).into_iter().collect();
        meta.contact = named(&described.contact_information);
        meta.references = named(&described.reference).into_iter().collect();
    }
    VrmDetails {
        version: VrmVersion::Legacy,
        meta,
        legacy: Some(license),
    }
}
