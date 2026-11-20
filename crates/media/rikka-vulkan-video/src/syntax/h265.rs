use std::ffi::c_void;
use std::ptr;

use ash::vk;
use ash::vk::native::*;

use crate::syntax::plain::{COLOUR_BT709, NO_SLOT, blank};
use crate::syntax::{Chain, Layout, PictureChain, Plan, RateChain, RateSettings};

const SMALLEST_CODING_BLOCK: u32 = 8;
const LOG2_MAX_ORDER_LSB: u8 = 16;
const LARGEST_DPB: u8 = 16;
const MOST_REFERENCES: u8 = LARGEST_DPB - 2;
const MERGE_CANDIDATES: u8 = 5;

const LEVELS: [(StdVideoH265LevelIdc, u64, u64); 13] = [
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_1_0,
        36_864,
        552_960,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_2_0,
        122_880,
        3_686_400,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_2_1,
        245_760,
        7_372_800,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_3_0,
        552_960,
        16_588_800,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_3_1,
        983_040,
        33_177_600,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_4_0,
        2_228_224,
        66_846_720,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_4_1,
        2_228_224,
        133_693_440,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_5_0,
        8_912_896,
        267_386_880,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_5_1,
        8_912_896,
        534_773_760,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_5_2,
        8_912_896,
        1_069_547_520,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_6_0,
        35_651_584,
        1_069_547_520,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_6_1,
        35_651_584,
        2_139_095_040,
    ),
    (
        StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_6_2,
        35_651_584,
        4_278_190_080,
    ),
];

pub(crate) fn coded(width: u32, height: u32, granularity: vk::Extent2D) -> vk::Extent2D {
    let across = SMALLEST_CODING_BLOCK.max(granularity.width);
    let down = SMALLEST_CODING_BLOCK.max(granularity.height);
    vk::Extent2D {
        width: width.next_multiple_of(across),
        height: height.next_multiple_of(down),
    }
}

pub(crate) fn most_references() -> u8 {
    MOST_REFERENCES
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Blocks {
    pub(crate) coding_log2: u8,
    pub(crate) smallest_transform_log2: u8,
    pub(crate) largest_transform_log2: u8,
}

impl Blocks {
    pub(crate) fn choose(
        ctb: vk::VideoEncodeH265CtbSizeFlagsKHR,
        transform: vk::VideoEncodeH265TransformBlockSizeFlagsKHR,
    ) -> Blocks {
        let coding_log2 = [
            (vk::VideoEncodeH265CtbSizeFlagsKHR::TYPE_32, 5),
            (vk::VideoEncodeH265CtbSizeFlagsKHR::TYPE_64, 6),
            (vk::VideoEncodeH265CtbSizeFlagsKHR::TYPE_16, 4),
        ]
        .into_iter()
        .find(|(flag, _)| ctb.contains(*flag))
        .map_or(5, |(_, log2)| log2);
        let sizes = [
            (vk::VideoEncodeH265TransformBlockSizeFlagsKHR::TYPE_4, 2),
            (vk::VideoEncodeH265TransformBlockSizeFlagsKHR::TYPE_8, 3),
            (vk::VideoEncodeH265TransformBlockSizeFlagsKHR::TYPE_16, 4),
            (vk::VideoEncodeH265TransformBlockSizeFlagsKHR::TYPE_32, 5),
        ];
        let supported = || sizes.iter().filter(|(flag, _)| transform.contains(*flag));
        let smallest = supported().map(|(_, log2)| *log2).min().unwrap_or(2);
        let largest = supported().map(|(_, log2)| *log2).max().unwrap_or(5);
        Blocks {
            coding_log2,
            smallest_transform_log2: smallest,
            largest_transform_log2: largest.min(coding_log2),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Syntax {
    layout: Layout,
    level: StdVideoH265LevelIdc,
    blocks: Blocks,
    syntax: vk::VideoEncodeH265StdFlagsKHR,
}

impl Syntax {
    pub(crate) fn new(
        layout: Layout,
        max_level: StdVideoH265LevelIdc,
        blocks: Blocks,
        syntax: vk::VideoEncodeH265StdFlagsKHR,
    ) -> Syntax {
        let samples = u64::from(layout.coded.width) * u64::from(layout.coded.height);
        let rate = samples * u64::from(layout.frame_rate.max(1));
        let pictures = u64::from(layout.references) + 1;
        let level = LEVELS
            .iter()
            .find(|(_, largest, per_second)| {
                let dpb = match samples {
                    size if size * 4 <= *largest => u64::from(LARGEST_DPB),
                    size if size * 2 <= *largest => 12,
                    size if size * 4 <= *largest * 3 => 8,
                    _ => 6,
                };
                samples <= *largest && rate <= *per_second && pictures <= dpb
            })
            .map_or(max_level, |(level, ..)| (*level).min(max_level));
        Syntax {
            layout,
            level,
            blocks,
            syntax,
        }
    }

    fn sample_adaptive_offset(&self) -> bool {
        self.syntax
            .contains(vk::VideoEncodeH265StdFlagsKHR::SAMPLE_ADAPTIVE_OFFSET_ENABLED_FLAG_SET)
    }

    pub(crate) fn parameters(&self) -> Box<dyn Chain> {
        let layout = &self.layout;
        let blocks = self.blocks;
        let mut parts = Box::new(Parameters {
            tier: blank(),
            buffering: blank(),
            vps: blank(),
            vui: blank(),
            sps: blank(),
            pps: blank(),
            add: vk::VideoEncodeH265SessionParametersAddInfoKHR::default(),
            create: vk::VideoEncodeH265SessionParametersCreateInfoKHR::default(),
        });
        parts.tier.general_profile_idc = StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_MAIN;
        parts.tier.general_level_idc = self.level;
        parts.tier.flags.set_general_progressive_source_flag(1);
        parts.tier.flags.set_general_frame_only_constraint_flag(1);
        parts.buffering.max_dec_pic_buffering_minus1[0] = layout.references;
        parts.buffering.max_num_reorder_pics[0] = 0;
        parts.buffering.max_latency_increase_plus1[0] = 0;
        parts.vps.flags.set_vps_temporal_id_nesting_flag(1);
        parts
            .vps
            .flags
            .set_vps_sub_layer_ordering_info_present_flag(1);
        let vui = &mut parts.vui;
        vui.flags.set_video_signal_type_present_flag(1);
        vui.flags.set_colour_description_present_flag(1);
        vui.video_format = 5;
        vui.colour_primaries = COLOUR_BT709;
        vui.transfer_characteristics = COLOUR_BT709;
        vui.matrix_coeffs = COLOUR_BT709;
        let sample_adaptive_offset = self.sample_adaptive_offset();
        let sps = &mut parts.sps;
        sps.flags.set_sps_temporal_id_nesting_flag(1);
        sps.flags.set_sps_sub_layer_ordering_info_present_flag(1);
        sps.flags.set_amp_enabled_flag(1);
        sps.flags
            .set_sample_adaptive_offset_enabled_flag(u32::from(sample_adaptive_offset));
        sps.flags.set_strong_intra_smoothing_enabled_flag(1);
        sps.flags.set_vui_parameters_present_flag(1);
        sps.chroma_format_idc = StdVideoH265ChromaFormatIdc_STD_VIDEO_H265_CHROMA_FORMAT_IDC_420;
        sps.pic_width_in_luma_samples = layout.coded.width;
        sps.pic_height_in_luma_samples = layout.coded.height;
        sps.log2_max_pic_order_cnt_lsb_minus4 = LOG2_MAX_ORDER_LSB - 4;
        sps.log2_min_luma_coding_block_size_minus3 = 0;
        sps.log2_diff_max_min_luma_coding_block_size = blocks.coding_log2 - 3;
        sps.log2_min_luma_transform_block_size_minus2 = blocks.smallest_transform_log2 - 2;
        sps.log2_diff_max_min_luma_transform_block_size =
            blocks.largest_transform_log2 - blocks.smallest_transform_log2;
        let depth = blocks.coding_log2 - blocks.smallest_transform_log2;
        sps.max_transform_hierarchy_depth_inter = depth;
        sps.max_transform_hierarchy_depth_intra = depth;
        let crop_right = (layout.coded.width - layout.picture.width) / 2;
        let crop_bottom = (layout.coded.height - layout.picture.height) / 2;
        if crop_right > 0 || crop_bottom > 0 {
            sps.flags.set_conformance_window_flag(1);
            sps.conf_win_right_offset = crop_right;
            sps.conf_win_bottom_offset = crop_bottom;
        }
        let transform_skip = self
            .syntax
            .contains(vk::VideoEncodeH265StdFlagsKHR::TRANSFORM_SKIP_ENABLED_FLAG_SET);
        let pps = &mut parts.pps;
        pps.flags.set_cu_qp_delta_enabled_flag(1);
        pps.flags
            .set_transform_skip_enabled_flag(u32::from(transform_skip));
        pps.flags.set_pps_loop_filter_across_slices_enabled_flag(1);
        parts.vps.pDecPicBufMgr = ptr::from_ref(&parts.buffering);
        parts.vps.pProfileTierLevel = ptr::from_ref(&parts.tier);
        parts.sps.pProfileTierLevel = ptr::from_ref(&parts.tier);
        parts.sps.pDecPicBufMgr = ptr::from_ref(&parts.buffering);
        parts.sps.pSequenceParameterSetVui = ptr::from_ref(&parts.vui);
        parts.add.std_vps_count = 1;
        parts.add.p_std_vp_ss = ptr::from_ref(&parts.vps);
        parts.add.std_sps_count = 1;
        parts.add.p_std_sp_ss = ptr::from_ref(&parts.sps);
        parts.add.std_pps_count = 1;
        parts.add.p_std_pp_ss = ptr::from_ref(&parts.pps);
        parts.create.max_std_vps_count = 1;
        parts.create.max_std_sps_count = 1;
        parts.create.max_std_pps_count = 1;
        parts.create.p_parameters_add_info = ptr::from_ref(&parts.add);
        parts
    }

    pub(crate) fn header_query(&self) -> Box<dyn Chain> {
        Box::new(HeaderQuery(
            vk::VideoEncodeH265SessionParametersGetInfoKHR::default()
                .write_std_vps(true)
                .write_std_sps(true)
                .write_std_pps(true),
        ))
    }

    pub(crate) fn rate(&self, rate: &RateSettings) -> Box<dyn RateChain> {
        let qp = |value: i32| vk::VideoEncodeH265QpKHR {
            qp_i: value,
            qp_p: value,
            qp_b: value,
        };
        let layer = vk::VideoEncodeH265RateControlLayerInfoKHR {
            use_min_qp: vk::TRUE,
            min_qp: qp(rate.min_qp),
            use_max_qp: vk::TRUE,
            max_qp: qp(rate.max_qp),
            use_max_frame_size: vk::TRUE,
            max_frame_size: vk::VideoEncodeH265FrameSizeKHR {
                frame_i_size: rate.max_intra_bytes,
                frame_p_size: rate.max_predicted_bytes,
                frame_b_size: rate.max_predicted_bytes,
            },
            ..Default::default()
        };
        Box::new(Rate {
            info: vk::VideoEncodeH265RateControlInfoKHR::default().sub_layer_count(1),
            layer,
        })
    }

    pub(crate) fn picture(&self, plan: &Plan) -> Box<dyn PictureChain> {
        let mut parts = Box::new(Picture {
            slice_header: blank(),
            slice: vk::VideoEncodeH265NaluSliceSegmentInfoKHR::default(),
            references: blank(),
            lists: blank(),
            std: blank(),
            info: vk::VideoEncodeH265PictureInfoKHR::default(),
            setup_std: blank(),
            setup: vk::VideoEncodeH265DpbSlotInfoKHR::default(),
            reference_std: blank(),
            reference: vk::VideoEncodeH265DpbSlotInfoKHR::default(),
        });
        let kind = picture_type(plan.intra);
        let order = order_count(plan.order);
        let sample_adaptive_offset = u32::from(self.sample_adaptive_offset());
        let header = &mut parts.slice_header;
        header.flags.set_first_slice_segment_in_pic_flag(1);
        header.flags.set_slice_sao_luma_flag(sample_adaptive_offset);
        header
            .flags
            .set_slice_sao_chroma_flag(sample_adaptive_offset);
        header
            .flags
            .set_slice_loop_filter_across_slices_enabled_flag(1);
        header.slice_type = match plan.intra {
            true => StdVideoH265SliceType_STD_VIDEO_H265_SLICE_TYPE_I,
            false => StdVideoH265SliceType_STD_VIDEO_H265_SLICE_TYPE_P,
        };
        header.MaxNumMergeCand = MERGE_CANDIDATES;
        parts.slice.constant_qp = plan.qp.unwrap_or(0);
        parts.std.flags.set_is_reference(1);
        parts.std.flags.set_IrapPicFlag(u32::from(plan.intra));
        parts.std.flags.set_pic_output_flag(1);
        parts.std.pic_type = kind;
        parts.std.PicOrderCntVal = order;
        parts.setup_std.pic_type = kind;
        parts.setup_std.PicOrderCntVal = order;
        if let Some(reference) = plan.reference {
            let mut kept = plan.kept.clone();
            kept.sort_by_key(|kept| std::cmp::Reverse(kept.order));
            kept.dedup_by_key(|kept| kept.order);
            kept.truncate(usize::from(MOST_REFERENCES));
            let rps = &mut parts.references;
            let mut previous = plan.order;
            for (index, held) in kept.iter().enumerate() {
                rps.delta_poc_s0_minus1[index] = (previous - held.order - 1) as u16;
                previous = held.order;
                if held.order == reference.order {
                    rps.used_by_curr_pic_s0_flag |= 1 << index;
                }
            }
            rps.num_negative_pics = kept.len() as u8;
            parts.lists.RefPicList0 = [NO_SLOT; 15];
            parts.lists.RefPicList1 = [NO_SLOT; 15];
            parts.lists.RefPicList0[0] = reference.slot;
            parts.reference_std.pic_type = picture_type(reference.intra);
            parts.reference_std.PicOrderCntVal = order_count(reference.order);
            parts.std.pRefLists = ptr::from_ref(&parts.lists);
        }
        parts.std.pShortTermRefPicSet = ptr::from_ref(&parts.references);
        parts.slice.p_std_slice_segment_header = ptr::from_ref(&parts.slice_header);
        parts.info.nalu_slice_segment_entry_count = 1;
        parts.info.p_nalu_slice_segment_entries = ptr::from_ref(&parts.slice);
        parts.info.p_std_picture_info = ptr::from_ref(&parts.std);
        parts.setup.p_std_reference_info = ptr::from_ref(&parts.setup_std);
        parts.reference.p_std_reference_info = ptr::from_ref(&parts.reference_std);
        parts
    }
}

fn picture_type(intra: bool) -> StdVideoH265PictureType {
    match intra {
        true => StdVideoH265PictureType_STD_VIDEO_H265_PICTURE_TYPE_IDR,
        false => StdVideoH265PictureType_STD_VIDEO_H265_PICTURE_TYPE_P,
    }
}

fn order_count(order: u32) -> i32 {
    (order & 0x7FFF_FFFF) as i32
}

struct Parameters {
    tier: StdVideoH265ProfileTierLevel,
    buffering: StdVideoH265DecPicBufMgr,
    vps: StdVideoH265VideoParameterSet,
    vui: StdVideoH265SequenceParameterSetVui,
    sps: StdVideoH265SequenceParameterSet,
    pps: StdVideoH265PictureParameterSet,
    add: vk::VideoEncodeH265SessionParametersAddInfoKHR<'static>,
    create: vk::VideoEncodeH265SessionParametersCreateInfoKHR<'static>,
}

unsafe impl Send for Parameters {}

impl Chain for Parameters {
    fn next(&self) -> *const c_void {
        ptr::from_ref(&self.create).cast()
    }
}

struct HeaderQuery(vk::VideoEncodeH265SessionParametersGetInfoKHR<'static>);

unsafe impl Send for HeaderQuery {}

impl Chain for HeaderQuery {
    fn next(&self) -> *const c_void {
        ptr::from_ref(&self.0).cast()
    }
}

struct Rate {
    info: vk::VideoEncodeH265RateControlInfoKHR<'static>,
    layer: vk::VideoEncodeH265RateControlLayerInfoKHR<'static>,
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
    slice_header: StdVideoEncodeH265SliceSegmentHeader,
    slice: vk::VideoEncodeH265NaluSliceSegmentInfoKHR<'static>,
    references: StdVideoH265ShortTermRefPicSet,
    lists: StdVideoEncodeH265ReferenceListsInfo,
    std: StdVideoEncodeH265PictureInfo,
    info: vk::VideoEncodeH265PictureInfoKHR<'static>,
    setup_std: StdVideoEncodeH265ReferenceInfo,
    setup: vk::VideoEncodeH265DpbSlotInfoKHR<'static>,
    reference_std: StdVideoEncodeH265ReferenceInfo,
    reference: vk::VideoEncodeH265DpbSlotInfoKHR<'static>,
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
