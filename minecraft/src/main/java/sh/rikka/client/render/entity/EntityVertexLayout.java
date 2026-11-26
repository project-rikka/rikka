package sh.rikka.client.render.entity;

import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.renderpearl.api.GpuFormat;
import com.mojang.renderpearl.api.vertex.VertexFormat;

final class EntityVertexLayout {
	private static final VertexFormat PACKED = VertexFormat.builder(0)
		.addAttribute("Position", GpuFormat.RGB32_FLOAT)
		.addAttribute("Color", GpuFormat.RGBA8_UNORM)
		.addAttribute("UV0", GpuFormat.RG32_FLOAT)
		.addAttribute("UV1", GpuFormat.RG16_SINT)
		.addAttribute("UV2", GpuFormat.RG16_SINT)
		.addAttribute("Normal", GpuFormat.RGBA8_SNORM)
		.build();

	static final boolean MATCHES_ENTITY_FORMAT = PACKED.equals(DefaultVertexFormat.ENTITY);
	static final int VERTEX_SIZE = PACKED.getVertexSize();
	static final long Z_COLOR_OFFSET = 8L;
	static final long UV_OFFSET = 16L;
	static final long OVERLAY_LIGHT_OFFSET = 24L;
	static final long NORMAL_OFFSET = 32L;

	private EntityVertexLayout() {
	}

	static boolean accepts(QuadVertexSink sink) {
		return MATCHES_ENTITY_FORMAT && sink.rikka$takesQuads(DefaultVertexFormat.ENTITY);
	}
}
