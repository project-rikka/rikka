package sh.rikka.client.render.skin;

import com.mojang.renderpearl.api.GpuFormat;
import com.mojang.renderpearl.api.pipeline.BindGroupLayout;
import com.mojang.renderpearl.api.pipeline.UniformType;
import com.mojang.renderpearl.api.vertex.VertexFormat;

public final class SkinLayout {
	public static final String PALETTE_UNIFORM = "SkinPalette";
	public static final BindGroupLayout PALETTE = BindGroupLayout.builder().withUniform(PALETTE_UNIFORM, UniformType.UNIFORM_BUFFER).build();
	public static final int ROW_BYTES = 16;
	public static final int WINDOW_ROWS = 1024;
	public static final int WINDOW_BYTES = WINDOW_ROWS * ROW_BYTES;
	public static final int BONE_ROWS = 6;
	public static final int BONE_BYTES = BONE_ROWS * ROW_BYTES;
	public static final int NORMAL_ROW_OFFSET = 3 * ROW_BYTES;
	public static final int NO_TINT = -1;

	public static final VertexFormat VERTEX = VertexFormat.builder(0)
		.addAttribute("Position", GpuFormat.RGB32_FLOAT)
		.addAttribute("UV0", GpuFormat.RG32_FLOAT)
		.addAttribute("Normal", GpuFormat.RGBA8_SNORM)
		.addAttribute("Slots", GpuFormat.RGBA16_SINT)
		.build();
	public static final int VERTEX_BYTES = VERTEX.getVertexSize();
	public static final int VERTEX_UV_OFFSET = 12;
	public static final int VERTEX_NORMAL_OFFSET = 20;
	public static final int VERTEX_SLOTS_OFFSET = 24;

	public static final VertexFormat INSTANCE = VertexFormat.builder(1)
		.addAttribute("Color", GpuFormat.RGBA8_UNORM)
		.addAttribute("UV1", GpuFormat.RG16_SINT)
		.addAttribute("UV2", GpuFormat.RG16_SINT)
		.addAttribute("PaletteBase", GpuFormat.R32_SINT)
		.addAttribute("UvTransform", GpuFormat.RGBA32_FLOAT)
		.build();
	public static final int INSTANCE_BYTES = INSTANCE.getVertexSize();
	public static final int INSTANCE_OVERLAY_OFFSET = 4;
	public static final int INSTANCE_LIGHT_OFFSET = 8;
	public static final int INSTANCE_BASE_OFFSET = 12;
	public static final int INSTANCE_UV_OFFSET = 16;

	public static final int VERTICES_PER_QUAD = 4;
	public static final int INDICES_PER_QUAD = 6;

	private SkinLayout() {
	}
}
