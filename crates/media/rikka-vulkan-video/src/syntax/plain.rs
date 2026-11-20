use ash::vk::native;

pub(crate) unsafe trait Plain: Copy {}

macro_rules! plain {
    ($($name:ident),+ $(,)?) => {
        $(unsafe impl Plain for native::$name {})+
    };
}

plain!(
    StdVideoH264SequenceParameterSetVui,
    StdVideoH264SequenceParameterSet,
    StdVideoH264PictureParameterSet,
    StdVideoEncodeH264RefListModEntry,
    StdVideoEncodeH264ReferenceListsInfo,
    StdVideoEncodeH264PictureInfo,
    StdVideoEncodeH264ReferenceInfo,
    StdVideoEncodeH264SliceHeader,
    StdVideoH265VideoParameterSet,
    StdVideoH265ProfileTierLevel,
    StdVideoH265DecPicBufMgr,
    StdVideoH265SequenceParameterSet,
    StdVideoH265SequenceParameterSetVui,
    StdVideoH265PictureParameterSet,
    StdVideoH265ShortTermRefPicSet,
    StdVideoEncodeH265PictureInfo,
    StdVideoEncodeH265ReferenceListsInfo,
    StdVideoEncodeH265SliceSegmentHeader,
    StdVideoEncodeH265ReferenceInfo,
);

pub(crate) fn blank<T: Plain>() -> T {
    unsafe { std::mem::zeroed() }
}

pub(crate) const COLOUR_BT709: u8 = 1;
pub(crate) const NO_SLOT: u8 = 0xFF;
