package sh.rikka.client.render.skin;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.model.geom.builders.UVPair;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import org.joml.Vector3fc;

public record ItemSkin(List<Part> parts, int tintCount) {
	public static final int MAX_TINTS = 16;
	private static final ItemSkin UNSUPPORTED = new ItemSkin(List.of(), 0);

	public record Part(RenderType renderType, RenderType glintRenderType, int baseVertex, int indexCount) {
	}

	public boolean isSupported() {
		return !this.parts.isEmpty();
	}

	public int rows() {
		return SkinLayout.BONE_ROWS + this.tintCount;
	}

	static ItemSkin build(List<BakedQuad> quads, SkinMeshPool pool, SkinPipelines pipelines) {
		int tintCount = 0;
		for (BakedQuad quad : quads) {
			BakedQuad.MaterialInfo material = quad.materialInfo();
			if (!isSkinnable(material.itemRenderType(), pipelines) || !isSkinnable(material.itemGlintRenderType(), pipelines)) {
				return UNSUPPORTED;
			}
			tintCount = Math.max(tintCount, material.tintIndex() + 1);
		}
		if (quads.isEmpty() || tintCount > MAX_TINTS) {
			return UNSUPPORTED;
		}
		int baseVertex = pool.allocate(quads.size() * SkinLayout.VERTICES_PER_QUAD);
		SkinVertexWriter writer = new SkinVertexWriter(pool.address(baseVertex));
		List<Part> parts = new ArrayList<>();
		int start = 0;
		for (int index = 0; index < quads.size(); index++) {
			BakedQuad quad = quads.get(index);
			write(quad, writer);
			BakedQuad.MaterialInfo material = quad.materialInfo();
			boolean last = index + 1 == quads.size();
			if (last || !sharesRenderTypes(material, quads.get(index + 1).materialInfo())) {
				parts.add(new Part(
					material.itemRenderType(),
					material.itemGlintRenderType(),
					baseVertex + start * SkinLayout.VERTICES_PER_QUAD,
					(index + 1 - start) * SkinLayout.INDICES_PER_QUAD
				));
				start = index + 1;
			}
		}
		return new ItemSkin(List.copyOf(parts), tintCount);
	}

	private static boolean isSkinnable(RenderType renderType, SkinPipelines pipelines) {
		return !renderType.sortOnUpload() && pipelines.supports(renderType.pipeline());
	}

	private static boolean sharesRenderTypes(BakedQuad.MaterialInfo first, BakedQuad.MaterialInfo second) {
		return first.itemRenderType() == second.itemRenderType() && first.itemGlintRenderType() == second.itemGlintRenderType();
	}

	private static void write(BakedQuad quad, SkinVertexWriter writer) {
		BakedQuad.MaterialInfo material = quad.materialInfo();
		int tintRow = material.tintIndex() >= 0 ? SkinLayout.BONE_ROWS + material.tintIndex() : SkinLayout.NO_TINT;
		Vector3fc normal = quad.direction().getUnitVec3f();
		for (int vertex = 0; vertex < BakedQuad.VERTEX_COUNT; vertex++) {
			Vector3fc position = quad.position(vertex);
			long uv = quad.packedUV(vertex);
			writer.put(position.x(), position.y(), position.z(), UVPair.unpackU(uv), UVPair.unpackV(uv), normal, 0, tintRow, material.lightEmission());
		}
	}
}
