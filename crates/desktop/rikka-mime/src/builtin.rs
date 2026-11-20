pub(crate) const GLOBS: &str = "\
50:image/png:*.png
50:image/jpeg:*.jpg
50:image/jpeg:*.jpeg
50:image/jpeg:*.jpe
50:image/jpeg:*.jfif
50:image/gif:*.gif
50:image/webp:*.webp
50:image/bmp:*.bmp
50:image/bmp:*.dib
50:image/tiff:*.tif
50:image/tiff:*.tiff
50:image/avif:*.avif
50:image/avif-sequence:*.avifs
50:image/heif:*.heif
50:image/heif:*.hif
50:image/heif-sequence:*.heifs
50:image/heic:*.heic
50:image/heic-sequence:*.heics
50:image/jxl:*.jxl
50:image/jp2:*.jp2
50:image/x-jp2-codestream:*.j2k
50:image/x-jp2-codestream:*.j2c
50:image/x-jp2-codestream:*.jpc
50:image/x-exr:*.exr
50:image/vnd.radiance:*.hdr
50:image/x-tga:*.tga
50:image/x-tga:*.icb
50:image/x-tga:*.vda
50:image/x-tga:*.vst
50:image/vnd.microsoft.icon:*.ico
50:image/vnd.adobe.photoshop:*.psd
50:image/vnd.adobe.photoshop:*.psb
50:image/qoi:*.qoi
50:image/ktx2:*.ktx2
50:image/vnd-ms.dds:*.dds
50:image/svg+xml:*.svg
50:image/svg+xml-compressed:*.svgz
50:image/x-portable-anymap:*.pnm
50:image/x-portable-bitmap:*.pbm
50:image/x-portable-graymap:*.pgm
50:image/x-portable-pixmap:*.ppm
50:audio/mpeg:*.mp3
50:audio/mpeg:*.mpga
50:audio/x-wav:*.wav
50:audio/flac:*.flac
50:audio/ogg:*.oga
50:audio/ogg:*.ogg
40:audio/x-vorbis+ogg:*.ogg
50:audio/x-opus+ogg:*.opus
50:audio/mp4:*.m4a
50:audio/mp4:*.m4b
50:audio/aac:*.aac
50:audio/aac:*.adts
50:audio/x-aiff:*.aif
50:audio/x-aiff:*.aiff
50:audio/x-aiff:*.aifc
50:audio/x-ms-wma:*.wma
50:audio/x-matroska:*.mka
50:audio/webm:*.weba
50:audio/x-ape:*.ape
50:audio/x-wavpack:*.wv
50:audio/midi:*.mid
50:audio/midi:*.midi
50:audio/x-dsf:*.dsf
50:audio/x-dff:*.dff
50:video/mp4:*.mp4
50:video/x-m4v:*.m4v
50:video/quicktime:*.mov
50:video/quicktime:*.qt
50:video/x-matroska:*.mkv
50:video/webm:*.webm
50:video/x-msvideo:*.avi
50:video/mpeg:*.mpg
50:video/mpeg:*.mpeg
50:video/mpeg:*.mpe
50:video/mp2t:*.ts
50:video/mp2t:*.m2ts
50:video/mp2t:*.mts
50:video/x-ms-wmv:*.wmv
50:video/x-flv:*.flv
50:video/3gpp:*.3gp
50:video/ogg:*.ogv
50:model/gltf-binary:*.glb
50:model/gltf+json:*.gltf
60:model/vrm:*.vrm
60:model/x.vrma:*.vrma
50:model/x.fbx:*.fbx
50:model/x.pmx:*.pmx
50:model/x.vmd:*.vmd
50:model/stl:*.stl
50:model/obj:*.obj
50:model/x.rikka-motion:*.rmotion
50:application/x-rikka-model:*.rmodel
50:application/x-blender:*.blend
50:application/vnd.iccprofile:*.icc
50:application/vnd.iccprofile:*.icm
50:application/json:*.json
50:application/xml:*.xml
50:application/pdf:*.pdf
50:application/zip:*.zip
50:text/plain:*.txt
";

pub(crate) const ALIASES: &str = "\
audio/wav audio/x-wav
audio/vnd.wave audio/x-wav
audio/x-flac audio/flac
audio/mp3 audio/mpeg
audio/x-mpeg audio/mpeg
audio/x-m4a audio/mp4
image/x-icon image/vnd.microsoft.icon
image/x-targa image/x-tga
image/x-qoi image/qoi
image/x-ms-bmp image/bmp
image/x-psd image/vnd.adobe.photoshop
model/x-pmx model/x.pmx
model/x-vrm model/vrm
application/x-fbx model/x.fbx
";

pub(crate) const SUBCLASSES: &str = "\
model/vrm model/gltf-binary
model/x.vrma model/gltf-binary
model/gltf+json application/json
model/x.rikka-motion application/json
audio/x-vorbis+ogg audio/ogg
audio/x-opus+ogg audio/ogg
image/svg+xml application/xml
application/xml text/plain
";

const SIGNATURE: &[u8] = b"MIME-Magic\0\n";

struct Match {
    indent: usize,
    offset: usize,
    range: usize,
    value: &'static [u8],
}

const fn at(offset: usize, value: &'static [u8]) -> Match {
    Match {
        indent: 0,
        offset,
        range: 1,
        value,
    }
}

const fn within(offset: usize, range: usize, value: &'static [u8]) -> Match {
    Match {
        indent: 0,
        offset,
        range,
        value,
    }
}

const fn then(offset: usize, value: &'static [u8]) -> Match {
    Match {
        indent: 1,
        offset,
        range: 1,
        value,
    }
}

const MAGIC: &[(u32, &str, &[Match])] = &[
    (50, "image/png", &[at(0, b"\x89PNG\r\n\x1a\n")]),
    (50, "image/jpeg", &[at(0, b"\xff\xd8\xff")]),
    (50, "image/gif", &[at(0, b"GIF87a"), at(0, b"GIF89a")]),
    (50, "image/webp", &[at(0, b"RIFF"), then(8, b"WEBP")]),
    (40, "image/bmp", &[at(0, b"BM")]),
    (50, "image/tiff", &[at(0, b"II*\0"), at(0, b"MM\0*")]),
    (50, "image/vnd.adobe.photoshop", &[at(0, b"8BPS")]),
    (50, "image/x-exr", &[at(0, b"\x76\x2f\x31\x01")]),
    (
        50,
        "image/jxl",
        &[at(0, b"\xff\x0a"), at(0, b"\0\0\0\x0cJXL \r\n\x87\n")],
    ),
    (50, "image/avif", &[at(4, b"ftypavif")]),
    (50, "image/avif-sequence", &[at(4, b"ftypavis")]),
    (50, "image/heic", &[at(4, b"ftypheic"), at(4, b"ftypheix")]),
    (50, "image/heif", &[at(4, b"ftypmif1")]),
    (50, "image/jp2", &[at(0, b"\0\0\0\x0cjP  \r\n\x87\n")]),
    (50, "image/x-jp2-codestream", &[at(0, b"\xff\x4f\xff\x51")]),
    (50, "image/qoi", &[at(0, b"qoif")]),
    (50, "image/ktx2", &[at(0, b"\xabKTX 20\xbb\r\n\x1a\n")]),
    (50, "image/vnd-ms.dds", &[at(0, b"DDS ")]),
    (
        50,
        "image/vnd.radiance",
        &[at(0, b"#?RADIANCE"), at(0, b"#?RGBE")],
    ),
    (40, "image/vnd.microsoft.icon", &[at(0, b"\0\0\x01\0")]),
    (50, "audio/flac", &[at(0, b"fLaC")]),
    (50, "audio/x-wav", &[at(0, b"RIFF"), then(8, b"WAVE")]),
    (
        50,
        "audio/x-aiff",
        &[at(0, b"FORM"), then(8, b"AIFF"), then(8, b"AIFC")],
    ),
    (50, "audio/ogg", &[at(0, b"OggS")]),
    (50, "audio/mpeg", &[at(0, b"ID3")]),
    (50, "audio/mp4", &[at(4, b"ftypM4A ")]),
    (50, "audio/x-ape", &[at(0, b"MAC ")]),
    (50, "audio/x-wavpack", &[at(0, b"wvpk")]),
    (50, "audio/midi", &[at(0, b"MThd")]),
    (50, "audio/x-dsf", &[at(0, b"DSD ")]),
    (50, "audio/x-dff", &[at(0, b"FRM8")]),
    (50, "video/x-matroska", &[at(0, b"\x1a\x45\xdf\xa3")]),
    (
        50,
        "video/mp4",
        &[
            at(4, b"ftypisom"),
            at(4, b"ftypiso2"),
            at(4, b"ftypmp41"),
            at(4, b"ftypmp42"),
            at(4, b"ftypavc1"),
        ],
    ),
    (50, "video/x-m4v", &[at(4, b"ftypM4V ")]),
    (50, "video/quicktime", &[at(4, b"ftypqt  "), at(4, b"moov")]),
    (50, "video/3gpp", &[at(4, b"ftyp3gp")]),
    (50, "video/x-msvideo", &[at(0, b"RIFF"), then(8, b"AVI ")]),
    (50, "video/x-flv", &[at(0, b"FLV\x01")]),
    (
        50,
        "video/x-ms-wmv",
        &[at(0, b"\x30\x26\xb2\x75\x8e\x66\xcf\x11")],
    ),
    (50, "model/gltf-binary", &[at(0, b"glTF")]),
    (60, "model/x.fbx", &[at(0, b"Kaydara FBX Binary")]),
    (60, "model/x.pmx", &[at(0, b"PMX ")]),
    (60, "model/x.vmd", &[at(0, b"Vocaloid Motion Data")]),
    (60, "application/x-rikka-model", &[at(0, b"RKMODEL\0")]),
    (
        60,
        "model/x.rikka-motion",
        &[within(0, 64, b"\"rikka-motion\"")],
    ),
    (50, "application/x-blender", &[at(0, b"BLENDER")]),
    (50, "application/vnd.iccprofile", &[at(36, b"acsp")]),
    (50, "application/pdf", &[at(0, b"%PDF-")]),
    (40, "application/zip", &[at(0, b"PK\x03\x04")]),
];

pub(crate) fn magic() -> Vec<u8> {
    let mut encoded = SIGNATURE.to_vec();
    for (priority, mime, matches) in MAGIC {
        encoded.extend_from_slice(format!("[{priority}:{mime}]\n").as_bytes());
        for found in *matches {
            if found.indent > 0 {
                encoded.extend_from_slice(found.indent.to_string().as_bytes());
            }
            encoded.extend_from_slice(format!(">{}=", found.offset).as_bytes());
            let length = u16::try_from(found.value.len()).unwrap_or(u16::MAX);
            encoded.extend_from_slice(&length.to_be_bytes());
            encoded.extend_from_slice(found.value);
            if found.range > 1 {
                encoded.extend_from_slice(format!("+{}", found.range).as_bytes());
            }
            encoded.push(b'\n');
        }
    }
    encoded
}
