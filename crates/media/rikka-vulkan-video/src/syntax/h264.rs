use std::ffi::c_void;
use std::ptr;

use ash::vk;
use ash::vk::native::*;

use crate::syntax::plain::{COLOUR_BT709, NO_SLOT, blank};
use crate::syntax::{Chain, Layout, PictureChain, Plan, RateChain, RateSettings, Reference};

const MACROBLOCK: u32 = 16;
const LOG2_MAX_FRAME_NUM: u8 = 16;
const MAX_FRAME_NUM: u32 = 1 << LOG2_MAX_FRAME_NUM;
const MOST_REFERENCES: u8 = 16;

const LEVELS: [(StdVideoH264LevelIdc, u64, u32, u32); 11] = [
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_3_1,
        108_000,
        3_600,
        18_000,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_3_2,
        216_000,
        5_120,
        20_480,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_4_0,
        245_760,
        8_192,
        32_768,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_4_1,
        245_760,
        8_192,
        32_768,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_4_2,
        522_240,
        8_704,
        34_816,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_5_0,
        589_824,
        22_080,
        110_400,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_5_1,
        983_040,
        36_864,
        184_320,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_5_2,
        2_073_600,
        36_864,
        184_320,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_6_0,
        4_177_920,
        139_264,
        696_320,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_6_1,
        8_355_840,
        139_264,
        696_320,
    ),
    (
        StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_6_2,
        16_711_680,
        139_264,
        696_320,
    ),
];

pub(crate) fn coded(width: u32, height: u32) -> vk::Extent2D {
    vk::Extent2D {
        width: width.next_multiple_of(MACROBLOCK),
        height: height.next_multiple_of(MACROBLOCK),
    }
}

pub(crate) fn most_references() -> u8 {
    MOST_REFERENCES
}

#[derive(Debug)]
pub(crate) struct Syntax {
    layout: Layout,
    level: StdVideoH264LevelIdc,
}

impl Syntax {
    pub(crate) fn new(layout: Layout, max_level: StdVideoH264LevelIdc) -> Syntax {
        let blocks = (layout.coded.width / MACROBLOCK) * (layout.coded.height / MACROBLOCK);
        let rate = u64::from(blocks) * u64::from(layout.frame_rate.max(1));
        let level = LEVELS
            .iter()
            .find(|(_, per_second, frame, dpb)| {
                rate <= *per_second
                    && blocks <= *frame
                    && (dpb / blocks.max(1)).min(16) >= u32::from(layout.references)
            })
            .map_or(max_level, |(level, ..)| (*level).min(max_level));
        Syntax { layout, level }
    }

    pub(crate) fn parameters(&self) -> Box<dyn Chain> {
        let layout = &self.layout;
        let mut parts = Box::new(Parameters {
            vui: blank(),
            sps: blank(),
            pps: blank(),
            add: vk::VideoEncodeH264SessionParametersAddInfoKHR::default(),
            create: vk::VideoEncodeH264SessionParametersCreateInfoKHR::default(),
        });
        let vui = &mut parts.vui;
        vui.flags.set_video_signal_type_present_flag(1);
        vui.flags.set_color_description_present_flag(1);
        vui.flags.set_bitstream_restriction_flag(1);
        vui.video_format = 5;
        vui.colour_primaries = COLOUR_BT709;
        vui.transfer_characteristics = COLOUR_BT709;
        vui.matrix_coefficients = COLOUR_BT709;
        vui.max_num_reorder_frames = 0;
        vui.max_dec_frame_buffering = layout.references;
        let sps = &mut parts.sps;
        sps.flags.set_direct_8x8_inference_flag(1);
        sps.flags.set_frame_mbs_only_flag(1);
        sps.flags.set_gaps_in_frame_num_value_allowed_flag(1);
        sps.flags.set_vui_parameters_present_flag(1);
        sps.profile_idc = StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_HIGH;
        sps.level_idc = self.level;
        sps.chroma_format_idc = StdVideoH264ChromaFormatIdc_STD_VIDEO_H264_CHROMA_FORMAT_IDC_420;
        sps.log2_max_frame_num_minus4 = LOG2_MAX_FRAME_NUM - 4;
        sps.pic_order_cnt_type = StdVideoH264PocType_STD_VIDEO_H264_POC_TYPE_2;
        sps.max_num_ref_frames = layout.references;
        sps.pic_width_in_mbs_minus1 = layout.coded.width / MACROBLOCK - 1;
        sps.pic_height_in_map_units_minus1 = layout.coded.height / MACROBLOCK - 1;
        let crop_right = (layout.coded.width - layout.picture.width) / 2;
        let crop_bottom = (layout.coded.height - layout.picture.height) / 2;
        if crop_right > 0 || crop_bottom > 0 {
            sps.flags.set_frame_cropping_flag(1);
            sps.frame_crop_right_offset = crop_right;
            sps.frame_crop_bottom_offset = crop_bottom;
        }
        let pps = &mut parts.pps;
        pps.flags.set_transform_8x8_mode_flag(1);
        pps.flags.set_deblocking_filter_control_present_flag(1);
        pps.flags.set_entropy_coding_mode_flag(1);
        parts.sps.pSequenceParameterSetVui = ptr::from_ref(&parts.vui);
        parts.add.std_sps_count = 1;
        parts.add.p_std_sp_ss = ptr::from_ref(&parts.sps);
        parts.add.std_pps_count = 1;
        parts.add.p_std_pp_ss = ptr::from_ref(&parts.pps);
        parts.create.max_std_sps_count = 1;
        parts.create.max_std_pps_count = 1;
        parts.create.p_parameters_add_info = ptr::from_ref(&parts.add);
        parts
    }

    pub(crate) fn header_query(&self) -> Box<dyn Chain> {
        Box::new(HeaderQuery(
            vk::VideoEncodeH264SessionParametersGetInfoKHR::default()
                .write_std_sps(true)
                .write_std_pps(true),
        ))
    }

    pub(crate) fn rate(&self, rate: &RateSettings) -> Box<dyn RateChain> {
        let qp = |value: i32| vk::VideoEncodeH264QpKHR {
            qp_i: value,
            qp_p: value,
            qp_b: value,
        };
        let layer = vk::VideoEncodeH264RateControlLayerInfoKHR {
            use_min_qp: vk::TRUE,
            min_qp: qp(rate.min_qp),
            use_max_qp: vk::TRUE,
            max_qp: qp(rate.max_qp),
            use_max_frame_size: vk::TRUE,
            max_frame_size: vk::VideoEncodeH264FrameSizeKHR {
                frame_i_size: rate.max_intra_bytes,
                frame_p_size: rate.max_predicted_bytes,
                frame_b_size: rate.max_predicted_bytes,
            },
            ..Default::default()
        };
        Box::new(Rate {
            info: vk::VideoEncodeH264RateControlInfoKHR::default().temporal_layer_count(1),
            layer,
        })
    }

    pub(crate) fn picture(&self, plan: &Plan) -> Box<dyn PictureChain> {
        let mut parts = Box::new(Picture {
            slice_header: blank(),
            slice: vk::VideoEncodeH264NaluSliceInfoKHR::default(),
            modifications: [blank(); 2],
            lists: blank(),
            std: blank(),
            info: vk::VideoEncodeH264PictureInfoKHR::default(),
            setup_std: blank(),
            setup: vk::VideoEncodeH264DpbSlotInfoKHR::default(),
            reference_std: blank(),
            reference: vk::VideoEncodeH264DpbSlotInfoKHR::default(),
        });
        let frame_num = plan.order % MAX_FRAME_NUM;
        let kind = picture_type(plan.intra);
        parts.slice_header.slice_type = match plan.intra {
            true => StdVideoH264SliceType_STD_VIDEO_H264_SLICE_TYPE_I,
            false => StdVideoH264SliceType_STD_VIDEO_H264_SLICE_TYPE_P,
        };
        parts.slice.constant_qp = plan.qp.unwrap_or(0);
        parts.std.flags.set_IdrPicFlag(u32::from(plan.intra));
        parts.std.flags.set_is_reference(1);
        parts.std.idr_pic_id = plan.idr_id;
        parts.std.primary_pic_type = kind;
        parts.std.frame_num = frame_num;
        parts.std.PicOrderCnt = order_count(plan.order);
        parts.setup_std.primary_pic_type = kind;
        parts.setup_std.FrameNum = frame_num;
        parts.setup_std.PicOrderCnt = order_count(plan.order);
        if let Some(reference) = plan.reference {
            fill_lists(&mut parts, plan.order, reference);
            parts.reference_std.primary_pic_type = picture_type(reference.intra);
            parts.reference_std.FrameNum = reference.order % MAX_FRAME_NUM;
            parts.reference_std.PicOrderCnt = order_count(reference.order);
            parts.std.pRefLists = ptr::from_ref(&parts.lists);
        }
        parts.slice.p_std_slice_header = ptr::from_ref(&parts.slice_header);
        parts.info.nalu_slice_entry_count = 1;
        parts.info.p_nalu_slice_entries = ptr::from_ref(&parts.slice);
        parts.info.p_std_picture_info = ptr::from_ref(&parts.std);
        parts.setup.p_std_reference_info = ptr::from_ref(&parts.setup_std);
        parts.reference.p_std_reference_info = ptr::from_ref(&parts.reference_std);
        parts
    }
}

fn fill_lists(parts: &mut Picture, order: u32, reference: Reference) {
    parts.lists.RefPicList0 = [NO_SLOT; 32];
    parts.lists.RefPicList1 = [NO_SLOT; 32];
    parts.lists.RefPicList0[0] = reference.slot;
    let distance = order.wrapping_sub(reference.order) % MAX_FRAME_NUM;
    if distance > 1 {
        parts.modifications[0].modification_of_pic_nums_idc =
            StdVideoH264ModificationOfPicNumsIdc_STD_VIDEO_H264_MODIFICATION_OF_PIC_NUMS_IDC_SHORT_TERM_SUBTRACT;
        parts.modifications[0].abs_diff_pic_num_minus1 = (distance - 1) as u16;
        parts.modifications[1].modification_of_pic_nums_idc =
            StdVideoH264ModificationOfPicNumsIdc_STD_VIDEO_H264_MODIFICATION_OF_PIC_NUMS_IDC_END;
        parts.lists.flags.set_ref_pic_list_modification_flag_l0(1);
        parts.lists.refList0ModOpCount = 2;
        parts.lists.pRefList0ModOperations = parts.modifications.as_ptr();
    }
}

fn picture_type(intra: bool) -> StdVideoH264PictureType {
    match intra {
        true => StdVideoH264PictureType_STD_VIDEO_H264_PICTURE_TYPE_IDR,
        false => StdVideoH264PictureType_STD_VIDEO_H264_PICTURE_TYPE_P,
    }
}

fn order_count(order: u32) -> i32 {
    (order.wrapping_mul(2) & 0x7FFF_FFFF) as i32
}

struct Parameters {
    vui: StdVideoH264SequenceParameterSetVui,
    sps: StdVideoH264SequenceParameterSet,
    pps: StdVideoH264PictureParameterSet,
    add: vk::VideoEncodeH264SessionParametersAddInfoKHR<'static>,
    create: vk::VideoEncodeH264SessionParametersCreateInfoKHR<'static>,
}

unsafe impl Send for Parameters {}

impl Chain for Parameters {
    fn next(&self) -> *const c_void {
        ptr::from_ref(&self.create).cast()
    }
}

struct HeaderQuery(vk::VideoEncodeH264SessionParametersGetInfoKHR<'static>);

unsafe impl Send for HeaderQuery {}

impl Chain for HeaderQuery {
    fn next(&self) -> *const c_void {
        ptr::from_ref(&self.0).cast()
    }
}

struct Rate {
    info: vk::VideoEncodeH264RateControlInfoKHR<'static>,
    layer: vk::VideoEncodeH264RateControlLayerInfoKHR<'static>,
}

unsafe impl Send for Rate {}

impl RateChain for Rate {
    fn info(&self) -> *const c_void {
        ptr::from_ref(&self.info).cast()
    }

    fn layer(&self) -> *const c_void {
        ptr::from_ref(&self.layer).cast()
    }
}

struct Picture {
    slice_header: StdVideoEncodeH264SliceHeader,
    slice: vk::VideoEncodeH264NaluSliceInfoKHR<'static>,
    modifications: [StdVideoEncodeH264RefListModEntry; 2],
    lists: StdVideoEncodeH264ReferenceListsInfo,
    std: StdVideoEncodeH264PictureInfo,
    info: vk::VideoEncodeH264PictureInfoKHR<'static>,
    setup_std: StdVideoEncodeH264ReferenceInfo,
    setup: vk::VideoEncodeH264DpbSlotInfoKHR<'static>,
    reference_std: StdVideoEncodeH264ReferenceInfo,
    reference: vk::VideoEncodeH264DpbSlotInfoKHR<'static>,
}

unsafe impl Send for Picture {}

impl PictureChain for Picture {
    fn info(&self) -> *const c_void {
        ptr::from_ref(&self.info).cast()
    }

    fn setup(&self) -> *const c_void {
        ptr::from_ref(&self.setup).cast()
    }

    fn reference(&self) -> *const c_void {
        ptr::from_ref(&self.reference).cast()
    }
}
