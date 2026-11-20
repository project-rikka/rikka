#![allow(clippy::unnecessary_cast)]

const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodec, name) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodec, long_name) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodec, r#type) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodec, id) == 20);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodec, capabilities) == 24);
const _: () = assert!(size_of::<crate::avcodec::AVCodecHWConfig>() == 12);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecHWConfig, pix_fmt) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecHWConfig, methods) == 4);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecHWConfig, device_type) == 8);
const _: () = assert!(size_of::<crate::avcodec::RcOverride>() == 16);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::RcOverride, start_frame) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::RcOverride, end_frame) == 4);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::RcOverride, qscale) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::RcOverride, quality_factor) == 12);
const _: () = assert!(size_of::<crate::avcodec::AVCodecContext>() == 864);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, av_class) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, log_level_offset) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, codec_type) == 12);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, codec) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, codec_id) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, codec_tag) == 28);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, priv_data) == 32);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, internal) == 40);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, opaque) == 48);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, bit_rate) == 56);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, flags) == 64);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, flags2) == 68);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, extradata) == 72);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, extradata_size) == 80);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, time_base) == 84);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, pkt_timebase) == 92);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, framerate) == 100);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, delay) == 108);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, width) == 112);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, height) == 116);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, coded_width) == 120);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, coded_height) == 124);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, sample_aspect_ratio) == 128);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, pix_fmt) == 136);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, sw_pix_fmt) == 140);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, color_primaries) == 144);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, color_trc) == 148);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, colorspace) == 152);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, color_range) == 156);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, chroma_sample_location) == 160);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, field_order) == 164);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, refs) == 168);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, has_b_frames) == 172);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, slice_flags) == 176);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, draw_horiz_band) == 184);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, get_format) == 192);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, max_b_frames) == 200);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, b_quant_factor) == 204);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, b_quant_offset) == 208);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, i_quant_factor) == 212);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, i_quant_offset) == 216);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, lumi_masking) == 220);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, temporal_cplx_masking) == 224);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, spatial_cplx_masking) == 228);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, p_masking) == 232);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, dark_masking) == 236);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, nsse_weight) == 240);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, me_cmp) == 244);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, me_sub_cmp) == 248);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, mb_cmp) == 252);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, ildct_cmp) == 256);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, dia_size) == 260);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, last_predictor_count) == 264);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, me_pre_cmp) == 268);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, pre_dia_size) == 272);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, me_subpel_quality) == 276);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, me_range) == 280);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, mb_decision) == 284);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, intra_matrix) == 288);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, inter_matrix) == 296);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, chroma_intra_matrix) == 304);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, intra_dc_precision) == 312);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, mb_lmin) == 316);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, mb_lmax) == 320);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, bidir_refine) == 324);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, keyint_min) == 328);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, gop_size) == 332);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, mv0_threshold) == 336);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, slices) == 340);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, sample_rate) == 344);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, sample_fmt) == 348);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, ch_layout) == 352);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, frame_size) == 376);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, block_align) == 380);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, cutoff) == 384);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, audio_service_type) == 388);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, request_sample_fmt) == 392);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, initial_padding) == 396);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, trailing_padding) == 400);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, seek_preroll) == 404);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, get_buffer2) == 408);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, bit_rate_tolerance) == 416);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, global_quality) == 420);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, compression_level) == 424);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, qcompress) == 428);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, qblur) == 432);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, qmin) == 436);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, qmax) == 440);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, max_qdiff) == 444);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, rc_buffer_size) == 448);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, rc_override_count) == 452);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, rc_override) == 456);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, rc_max_rate) == 464);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, rc_min_rate) == 472);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, rc_max_available_vbv_use) == 480);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, rc_min_vbv_overflow_use) == 484);
const _: () = assert!(
    std::mem::offset_of!(crate::avcodec::AVCodecContext, rc_initial_buffer_occupancy) == 488
);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, trellis) == 492);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, stats_out) == 496);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, stats_in) == 504);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, workaround_bugs) == 512);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, strict_std_compliance) == 516);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, error_concealment) == 520);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, debug) == 524);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, err_recognition) == 528);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, hwaccel) == 536);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, hwaccel_context) == 544);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, hw_frames_ctx) == 552);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, hw_device_ctx) == 560);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, hwaccel_flags) == 568);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, extra_hw_frames) == 572);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, error) == 576);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, dct_algo) == 640);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, idct_algo) == 644);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, bits_per_coded_sample) == 648);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, bits_per_raw_sample) == 652);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, thread_count) == 656);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, thread_type) == 660);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, active_thread_type) == 664);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, execute) == 672);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, execute2) == 680);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, profile) == 688);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, level) == 692);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, skip_loop_filter) == 696);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, skip_idct) == 700);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, skip_frame) == 704);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, skip_alpha) == 708);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, skip_top) == 712);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, skip_bottom) == 716);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, lowres) == 720);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, codec_descriptor) == 728);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, sub_charenc) == 736);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, sub_charenc_mode) == 744);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, subtitle_header_size) == 748);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, subtitle_header) == 752);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, dump_separator) == 760);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, codec_whitelist) == 768);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, coded_side_data) == 776);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, nb_coded_side_data) == 784);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, export_side_data) == 788);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, max_pixels) == 792);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, apply_cropping) == 800);
const _: () = assert!(
    std::mem::offset_of!(crate::avcodec::AVCodecContext, discard_damaged_percentage) == 804
);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, max_samples) == 808);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, get_encode_buffer) == 816);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, frame_num) == 824);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, side_data_prefer_packet) == 832);
const _: () = assert!(
    std::mem::offset_of!(crate::avcodec::AVCodecContext, nb_side_data_prefer_packet) == 840
);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, decoded_side_data) == 848);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, nb_decoded_side_data) == 856);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecContext, alpha_mode) == 860);
const _: () = assert!(size_of::<crate::avcodec::AVCodecParameters>() == 184);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, codec_type) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, codec_id) == 4);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, codec_tag) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, extradata) == 16);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, extradata_size) == 24);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, coded_side_data) == 32);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, nb_coded_side_data) == 40);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, format) == 44);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, bit_rate) == 48);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, bits_per_coded_sample) == 56);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, bits_per_raw_sample) == 60);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, profile) == 64);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, level) == 68);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, width) == 72);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, height) == 76);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, sample_aspect_ratio) == 80);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, framerate) == 88);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, field_order) == 96);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, color_range) == 100);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, color_primaries) == 104);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, color_trc) == 108);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, color_space) == 112);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, chroma_location) == 116);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, video_delay) == 120);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, ch_layout) == 128);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, sample_rate) == 152);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, block_align) == 156);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, frame_size) == 160);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, initial_padding) == 164);
const _: () =
    assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, trailing_padding) == 168);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, seek_preroll) == 172);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecParameters, alpha_mode) == 176);
const _: () = assert!(size_of::<crate::avcodec::AVPacketSideData>() == 24);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacketSideData, data) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacketSideData, size) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacketSideData, r#type) == 16);
const _: () = assert!(size_of::<crate::avcodec::AVPacket>() == 104);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, buf) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, pts) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, dts) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, data) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, size) == 32);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, stream_index) == 36);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, flags) == 40);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, side_data) == 48);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, side_data_elems) == 56);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, duration) == 64);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, pos) == 72);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, opaque) == 80);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, opaque_ref) == 88);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVPacket, time_base) == 96);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_NONE as i64 == 0);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_H264 as i64 == 27);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_VP8 as i64 == 139);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_VP9 as i64 == 166);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_HEVC as i64 == 172);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_AV1 as i64 == 222);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_JPEG2000 as i64 == 88);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_TIFF as i64 == 96);
const _: () = assert!(crate::avcodec::AV_CODEC_ID_EXR as i64 == 177);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_NONE as i64 == -1);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_U8 as i64 == 0);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_S16 as i64 == 1);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_S32 as i64 == 2);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_FLT as i64 == 3);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_DBL as i64 == 4);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_U8P as i64 == 5);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_S16P as i64 == 6);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_S32P as i64 == 7);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_FLTP as i64 == 8);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_DBLP as i64 == 9);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_S64 as i64 == 10);
const _: () = assert!(crate::avcodec::AV_SAMPLE_FMT_S64P as i64 == 11);
const _: () = assert!(crate::avcodec::AV_PROFILE_AAC_HE as i64 == 4);
const _: () = assert!(crate::avcodec::AV_PROFILE_AAC_HE_V2 as i64 == 28);
const _: () = assert!(crate::avcodec::AV_CODEC_PROP_INTRA_ONLY as i64 == 1);
const _: () = assert!(crate::avcodec::AV_CODEC_PROP_LOSSY as i64 == 2);
const _: () = assert!(crate::avcodec::AV_CODEC_PROP_LOSSLESS as i64 == 4);
const _: () = assert!(crate::avcodec::AVDISCARD_DEFAULT as i64 == 0);
const _: () = assert!(crate::avcodec::AVDISCARD_ALL as i64 == 48);
const _: () = assert!(crate::avcodec::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i64 == 1);
const _: () = assert!(crate::avcodec::AV_CODEC_HW_CONFIG_METHOD_HW_FRAMES_CTX as i64 == 2);
const _: () = assert!(crate::avcodec::AV_CODEC_HW_CONFIG_METHOD_INTERNAL as i64 == 4);
const _: () = assert!(crate::avcodec::AV_CODEC_HW_CONFIG_METHOD_AD_HOC as i64 == 8);
const _: () = assert!(crate::avcodec::AV_CODEC_FLAG_OUTPUT_CORRUPT as i64 == 8);
const _: () = assert!(crate::avcodec::AV_CODEC_FLAG_LOW_DELAY as i64 == 524288);
const _: () = assert!(crate::avcodec::AV_CODEC_FLAG_GLOBAL_HEADER as i64 == 4194304);
const _: () = assert!(crate::avcodec::AV_EF_CRCCHECK as i64 == 1);
const _: () = assert!(crate::avcodec::AV_EF_BITSTREAM as i64 == 2);
const _: () = assert!(crate::avcodec::AV_EF_BUFFER as i64 == 4);
const _: () = assert!(crate::avcodec::AV_EF_EXPLODE as i64 == 8);
const _: () = assert!(crate::avcodec::AV_CODEC_CAP_HARDWARE as i64 == 262144);
const _: () = assert!(crate::avcodec::AV_CODEC_CAP_ENCODER_FLUSH as i64 == 2097152);
const _: () = assert!(crate::avcodec::AV_PKT_FLAG_KEY as i64 == 1);
const _: () = assert!(crate::avcodec::AV_INPUT_BUFFER_PADDING_SIZE as i64 == 64);
const _: () = assert!(crate::avcodec::FF_THREAD_FRAME as i64 == 1);
const _: () = assert!(crate::avcodec::FF_THREAD_SLICE as i64 == 2);
const _: () = assert!(crate::avcodec::AV_PKT_DATA_DISPLAYMATRIX as i64 == 5);
const _: () = assert!(crate::avcodec::AV_PKT_DATA_SKIP_SAMPLES as i64 == 11);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecDescriptor, id) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecDescriptor, r#type) == 4);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecDescriptor, name) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecDescriptor, long_name) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avcodec::AVCodecDescriptor, props) == 24);
const _: () = assert!(size_of::<crate::avformat::AVFormatContext>() == 480);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, av_class) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, iformat) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, oformat) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, priv_data) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, pb) == 32);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, ctx_flags) == 40);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, nb_streams) == 44);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, streams) == 48);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, nb_stream_groups) == 56);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, stream_groups) == 64);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, nb_chapters) == 72);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, chapters) == 80);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, url) == 88);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, start_time) == 96);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, duration) == 104);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, bit_rate) == 112);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, packet_size) == 120);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_delay) == 124);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, flags) == 128);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, probesize) == 136);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_analyze_duration) == 144);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, key) == 152);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, keylen) == 160);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, nb_programs) == 164);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, programs) == 168);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, video_codec_id) == 176);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, audio_codec_id) == 180);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, subtitle_codec_id) == 184);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, data_codec_id) == 188);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, metadata) == 192);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, start_time_realtime) == 200);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, fps_probe_size) == 208);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, error_recognition) == 212);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, interrupt_callback) == 216);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, debug) == 232);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_streams) == 236);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_index_size) == 240);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_picture_buffer) == 244);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_interleave_delta) == 248);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_ts_probe) == 256);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_chunk_duration) == 260);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_chunk_size) == 264);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, max_probe_packets) == 268);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, strict_std_compliance) == 272);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, event_flags) == 276);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, avoid_negative_ts) == 280);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, audio_preload) == 284);
const _: () = assert!(
    std::mem::offset_of!(
        crate::avformat::AVFormatContext,
        use_wallclock_as_timestamps
    ) == 288
);
const _: () = assert!(
    std::mem::offset_of!(
        crate::avformat::AVFormatContext,
        skip_estimate_duration_from_pts
    ) == 292
);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, avio_flags) == 296);
const _: () = assert!(
    std::mem::offset_of!(crate::avformat::AVFormatContext, duration_estimation_method) == 300
);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, skip_initial_bytes) == 304);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, correct_ts_overflow) == 312);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, seek2any) == 316);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, flush_packets) == 320);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, probe_score) == 324);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, format_probesize) == 328);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, codec_whitelist) == 336);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, format_whitelist) == 344);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, protocol_whitelist) == 352);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, protocol_blacklist) == 360);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, io_repositioned) == 368);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, video_codec) == 376);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, audio_codec) == 384);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, subtitle_codec) == 392);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, data_codec) == 400);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, metadata_header_padding) == 408);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, opaque) == 416);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, control_message_cb) == 424);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, output_ts_offset) == 432);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, dump_separator) == 440);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, io_open) == 448);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, io_close2) == 456);
const _: () =
    assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, duration_probesize) == 464);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVFormatContext, name) == 472);
const _: () = assert!(size_of::<crate::avformat::AVStream>() == 216);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, av_class) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, index) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, id) == 12);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, codecpar) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, priv_data) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, time_base) == 32);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, start_time) == 40);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, duration) == 48);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, nb_frames) == 56);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, disposition) == 64);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, discard) == 68);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, sample_aspect_ratio) == 72);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, metadata) == 80);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, avg_frame_rate) == 88);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, attached_pic) == 96);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, event_flags) == 200);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, r_frame_rate) == 204);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVStream, pts_wrap_bits) == 212);
const _: () = assert!(size_of::<crate::avformat::AVIOInterruptCB>() == 16);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIOInterruptCB, callback) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIOInterruptCB, opaque) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIndexEntry, pos) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIndexEntry, timestamp) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVOutputFormat, name) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVOutputFormat, long_name) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVOutputFormat, mime_type) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVOutputFormat, extensions) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVOutputFormat, audio_codec) == 32);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVOutputFormat, video_codec) == 36);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVOutputFormat, subtitle_codec) == 40);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVOutputFormat, flags) == 44);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVInputFormat, name) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVInputFormat, long_name) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVInputFormat, flags) == 16);
const _: () = assert!(crate::avformat::AVFMT_NOFILE as i64 == 1);
const _: () = assert!(crate::avformat::AVFMT_GLOBALHEADER as i64 == 64);
const _: () = assert!(crate::avformat::AVFMT_VARIABLE_FPS as i64 == 1024);
const _: () = assert!(crate::avformat::AVSEEK_FLAG_BACKWARD as i64 == 1);
const _: () = assert!(crate::avformat::AVSEEK_FLAG_BYTE as i64 == 2);
const _: () = assert!(crate::avformat::AVSEEK_FLAG_ANY as i64 == 4);
const _: () = assert!(crate::avformat::AVSEEK_FLAG_FRAME as i64 == 8);
const _: () = assert!(crate::avformat::AVIO_FLAG_READ as i64 == 1);
const _: () = assert!(crate::avformat::AVIO_FLAG_WRITE as i64 == 2);
const _: () = assert!(crate::avformat::AVIO_FLAG_READ_WRITE as i64 == 3);
const _: () = assert!(crate::avformat::AV_DISPOSITION_ATTACHED_PIC as i64 == 1024);
const _: () = assert!(crate::avformat::AV_DISPOSITION_DEFAULT as i64 == 1);
const _: () = assert!(crate::avformat::AVFMT_FLAG_GENPTS as i64 == 1);
const _: () = assert!(crate::avformat::AVFMT_FLAG_CUSTOM_IO as i64 == 128);
const _: () = assert!(crate::avformat::AVSEEK_SIZE as i64 == 65536);
const _: () = assert!(crate::avformat::AVSEEK_FORCE as i64 == 131072);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIOContext, av_class) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIOContext, buffer) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIOContext, buffer_size) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIOContext, buf_ptr) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIOContext, buf_end) == 32);
const _: () = assert!(std::mem::offset_of!(crate::avformat::AVIOContext, opaque) == 40);
const _: () = assert!(crate::swscale::SWS_FAST_BILINEAR as i64 == 1);
const _: () = assert!(crate::swscale::SWS_BILINEAR as i64 == 2);
const _: () = assert!(crate::swscale::SWS_BICUBIC as i64 == 4);
const _: () = assert!(crate::swscale::SWS_LANCZOS as i64 == 512);
const _: () = assert!(crate::swscale::SWS_FULL_CHR_H_INT as i64 == 8192);
const _: () = assert!(crate::swscale::SWS_FULL_CHR_H_INP as i64 == 16384);
const _: () = assert!(crate::swscale::SWS_ACCURATE_RND as i64 == 262144);
const _: () = assert!(size_of::<crate::avutil::AVRational>() == 8);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVRational, num) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVRational, den) == 4);
const _: () = assert!(size_of::<crate::avutil::AVBufferRef>() == 24);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVBufferRef, buffer) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVBufferRef, data) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVBufferRef, size) == 16);
const _: () = assert!(size_of::<crate::avutil::AVDictionaryEntry>() == 16);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVDictionaryEntry, key) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVDictionaryEntry, value) == 8);
const _: () = assert!(size_of::<crate::avutil::AVChannelLayout>() == 24);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVChannelLayout, order) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVChannelLayout, nb_channels) == 4);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVChannelLayout, u) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVChannelLayout, opaque) == 16);
const _: () = assert!(size_of::<crate::avutil::AVFrameSideData>() == 40);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrameSideData, r#type) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrameSideData, data) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrameSideData, size) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrameSideData, metadata) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrameSideData, buf) == 32);
const _: () = assert!(size_of::<crate::avutil::AVFrame>() == 424);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, data) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, linesize) == 64);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, extended_data) == 96);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, width) == 104);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, height) == 108);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, nb_samples) == 112);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, format) == 116);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, pict_type) == 120);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, sample_aspect_ratio) == 124);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, pts) == 136);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, pkt_dts) == 144);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, time_base) == 152);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, quality) == 160);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, opaque) == 168);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, repeat_pict) == 176);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, sample_rate) == 180);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, buf) == 184);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, extended_buf) == 248);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, nb_extended_buf) == 256);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, side_data) == 264);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, nb_side_data) == 272);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, flags) == 276);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, color_range) == 280);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, color_primaries) == 284);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, color_trc) == 288);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, colorspace) == 292);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, chroma_location) == 296);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, best_effort_timestamp) == 304);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, metadata) == 312);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, decode_error_flags) == 320);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, hw_frames_ctx) == 328);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, opaque_ref) == 336);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, crop_top) == 344);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, crop_bottom) == 352);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, crop_left) == 360);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, crop_right) == 368);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, private_ref) == 376);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, ch_layout) == 384);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, duration) == 408);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVFrame, alpha_mode) == 416);
const _: () = assert!(size_of::<crate::avutil::AVHWDeviceContext>() == 40);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWDeviceContext, av_class) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWDeviceContext, r#type) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWDeviceContext, hwctx) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWDeviceContext, free) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWDeviceContext, user_opaque) == 32);
const _: () = assert!(size_of::<crate::avutil::AVHWFramesContext>() == 80);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, av_class) == 0);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, device_ref) == 8);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, device_ctx) == 16);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, hwctx) == 24);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, free) == 32);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, user_opaque) == 40);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, pool) == 48);
const _: () =
    assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, initial_pool_size) == 56);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, format) == 60);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, sw_format) == 64);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, width) == 68);
const _: () = assert!(std::mem::offset_of!(crate::avutil::AVHWFramesContext, height) == 72);
const _: () = assert!(crate::avutil::AV_NUM_DATA_POINTERS as i64 == 8);
const _: () = assert!(crate::avutil::AVMEDIA_TYPE_UNKNOWN as i64 == -1);
const _: () = assert!(crate::avutil::AVMEDIA_TYPE_VIDEO as i64 == 0);
const _: () = assert!(crate::avutil::AVMEDIA_TYPE_AUDIO as i64 == 1);
const _: () = assert!(crate::avutil::AVMEDIA_TYPE_DATA as i64 == 2);
const _: () = assert!(crate::avutil::AVMEDIA_TYPE_SUBTITLE as i64 == 3);
const _: () = assert!(crate::avutil::AVMEDIA_TYPE_ATTACHMENT as i64 == 4);
const _: () = assert!(crate::avutil::AV_PICTURE_TYPE_NONE as i64 == 0);
const _: () = assert!(crate::avutil::AV_PICTURE_TYPE_I as i64 == 1);
const _: () = assert!(crate::avutil::AV_PIX_FMT_NONE as i64 == -1);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YUV420P as i64 == 0);
const _: () = assert!(crate::avutil::AV_PIX_FMT_RGB24 as i64 == 2);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YUV422P as i64 == 4);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YUV444P as i64 == 5);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GRAY8 as i64 == 8);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YUVJ420P as i64 == 12);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YUVJ422P as i64 == 13);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YUVJ444P as i64 == 14);
const _: () = assert!(crate::avutil::AV_PIX_FMT_NV12 as i64 == 23);
const _: () = assert!(crate::avutil::AV_PIX_FMT_RGBA as i64 == 26);
const _: () = assert!(crate::avutil::AV_PIX_FMT_BGRA as i64 == 28);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YUV420P10LE as i64 == 62);
const _: () = assert!(crate::avutil::AV_PIX_FMT_P010LE as i64 == 158);
const _: () = assert!(crate::avutil::AV_PIX_FMT_P016LE as i64 == 169);
const _: () = assert!(crate::avutil::AV_PIX_FMT_VULKAN as i64 == 190);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GRAY16LE as i64 == 30);
const _: () = assert!(crate::avutil::AV_PIX_FMT_RGB48LE as i64 == 35);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YA8 as i64 == 56);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YUV444P10LE as i64 == 68);
const _: () = assert!(crate::avutil::AV_PIX_FMT_RGBA64LE as i64 == 105);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YA16LE as i64 == 110);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GRAY10LE as i64 == 168);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GBRPF32BE as i64 == 174);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GBRPF32LE as i64 == 175);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GBRAPF32BE as i64 == 176);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GBRAPF32LE as i64 == 177);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GRAYF32LE as i64 == 183);
const _: () = assert!(crate::avutil::AV_PIX_FMT_RGBF32BE as i64 == 217);
const _: () = assert!(crate::avutil::AV_PIX_FMT_RGBF32LE as i64 == 218);
const _: () = assert!(crate::avutil::AV_PIX_FMT_RGBAF32BE as i64 == 219);
const _: () = assert!(crate::avutil::AV_PIX_FMT_RGBAF32LE as i64 == 220);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GBRPF16LE as i64 == 244);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GBRAPF16LE as i64 == 246);
const _: () = assert!(crate::avutil::AV_PIX_FMT_GRAYF16LE as i64 == 248);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YAF32LE as i64 == 253);
const _: () = assert!(crate::avutil::AV_PIX_FMT_YAF16LE as i64 == 255);
const _: () = assert!(crate::avutil::AVCOL_SPC_RGB as i64 == 0);
const _: () = assert!(crate::avutil::AVCOL_SPC_BT709 as i64 == 1);
const _: () = assert!(crate::avutil::AVCOL_SPC_UNSPECIFIED as i64 == 2);
const _: () = assert!(crate::avutil::AVCOL_SPC_BT470BG as i64 == 5);
const _: () = assert!(crate::avutil::AVCOL_SPC_SMPTE170M as i64 == 6);
const _: () = assert!(crate::avutil::AVCOL_SPC_SMPTE240M as i64 == 7);
const _: () = assert!(crate::avutil::AVCOL_SPC_BT2020_NCL as i64 == 9);
const _: () = assert!(crate::avutil::AVCOL_SPC_BT2020_CL as i64 == 10);
const _: () = assert!(crate::avutil::AVCOL_RANGE_UNSPECIFIED as i64 == 0);
const _: () = assert!(crate::avutil::AVCOL_RANGE_MPEG as i64 == 1);
const _: () = assert!(crate::avutil::AVCOL_RANGE_JPEG as i64 == 2);
const _: () = assert!(crate::avutil::AVCOL_PRI_BT709 as i64 == 1);
const _: () = assert!(crate::avutil::AVCOL_PRI_UNSPECIFIED as i64 == 2);
const _: () = assert!(crate::avutil::AVCOL_TRC_BT709 as i64 == 1);
const _: () = assert!(crate::avutil::AVCOL_TRC_UNSPECIFIED as i64 == 2);
const _: () = assert!(crate::avutil::AVCOL_TRC_LINEAR as i64 == 8);
const _: () = assert!(crate::avutil::AVCOL_TRC_IEC61966_2_1 as i64 == 13);
const _: () = assert!(crate::avutil::AV_FRAME_DATA_ICC_PROFILE as i64 == 15);
const _: () = assert!(crate::avutil::AV_PIX_FMT_VAAPI as i64 == 44);
const _: () = assert!(crate::avutil::AV_HWDEVICE_TYPE_VAAPI as i64 == 3);
const _: () = assert!(crate::avutil::AV_LOG_VERBOSE as i64 == 40);
const _: () = assert!(crate::avutil::AV_LOG_DEBUG as i64 == 48);
const _: () = assert!(crate::avutil::AVCHROMA_LOC_UNSPECIFIED as i64 == 0);
const _: () = assert!(crate::avutil::AVCHROMA_LOC_LEFT as i64 == 1);
const _: () = assert!(crate::avutil::AVCHROMA_LOC_CENTER as i64 == 2);
const _: () = assert!(crate::avutil::AVCHROMA_LOC_TOPLEFT as i64 == 3);
const _: () = assert!(crate::avutil::AV_HWDEVICE_TYPE_NONE as i64 == 0);
const _: () = assert!(crate::avutil::AV_HWDEVICE_TYPE_VULKAN as i64 == 11);
const _: () = assert!(crate::avutil::AV_HWFRAME_TRANSFER_DIRECTION_FROM as i64 == 0);
const _: () = assert!(crate::avutil::AV_HWFRAME_TRANSFER_DIRECTION_TO as i64 == 1);
const _: () = assert!(crate::avutil::AVERROR_EOF as i64 == -541478725);
const _: () = assert!(crate::avutil::AVERROR_DECODER_NOT_FOUND as i64 == -1128613112);
const _: () = assert!(crate::avutil::AVERROR_ENCODER_NOT_FOUND as i64 == -1129203192);
const _: () = assert!(crate::avutil::AVERROR_INVALIDDATA as i64 == -1094995529);
const _: () = assert!(crate::avutil::AVERROR_UNKNOWN as i64 == -1313558101);
const _: () = assert!(crate::avutil::AVERROR_EAGAIN as i64 == -11);
const _: () = assert!(crate::avutil::AVERROR_ENOMEM as i64 == -12);
const _: () = assert!(crate::avutil::AVERROR_EINVAL as i64 == -22);
const _: () = assert!(crate::avutil::AVERROR_EIO as i64 == -5);
const _: () = assert!(crate::avutil::AV_ERROR_MAX_STRING_SIZE as i64 == 64);
const _: () = assert!(crate::avutil::AV_NOPTS_VALUE as i64 == -9223372036854775808);
const _: () = assert!(crate::avutil::AV_TIME_BASE as i64 == 1000000);
const _: () = assert!(crate::avutil::AV_LOG_QUIET as i64 == -8);
const _: () = assert!(crate::avutil::AV_LOG_ERROR as i64 == 16);
const _: () = assert!(crate::avutil::AV_LOG_WARNING as i64 == 24);
const _: () = assert!(crate::avutil::AV_LOG_INFO as i64 == 32);
const _: () = assert!(crate::avutil::AV_OPT_SEARCH_CHILDREN as i64 == 1);
const _: () = assert!(crate::avutil::AV_FRAME_FLAG_CORRUPT as i64 == 1);
const _: () = assert!(crate::avutil::AV_FRAME_FLAG_KEY as i64 == 2);
const _: () = assert!(crate::avutil::AV_FRAME_FLAG_INTERLACED as i64 == 8);
const _: () = assert!(crate::avutil::AV_ROUND_ZERO as i64 == 0);
const _: () = assert!(crate::avutil::AV_ROUND_INF as i64 == 1);
const _: () = assert!(crate::avutil::AV_ROUND_DOWN as i64 == 2);
const _: () = assert!(crate::avutil::AV_ROUND_UP as i64 == 3);
const _: () = assert!(crate::avutil::AV_ROUND_NEAR_INF as i64 == 5);
const _: () = assert!(crate::avutil::AV_ROUND_PASS_MINMAX as i64 == 8192);
