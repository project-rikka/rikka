package sh.rikka.client.render.entity;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.renderer.SpriteCoordinateExpander;
import net.minecraft.client.renderer.texture.UvMapping;
import net.minecraft.util.ARGB;
import org.joml.Quaternionf;
import org.jspecify.annotations.Nullable;
import org.lwjgl.system.MemoryUtil;

public final class ModelPartRenderer {
	public static final float PART_UNITS = 16.0F;
	private static final ThreadLocal<ModelPartRenderer> RENDERERS = ThreadLocal.withInitial(ModelPartRenderer::new);

	private final Quaternionf rotation = new Quaternionf();
	private final PartTransform transform = new PartTransform();
	private long[] cornerXY = new long[8];
	private long[] cornerZColor = new long[8];
	private @Nullable VertexConsumer buffer;
	private @Nullable QuadVertexSink sink;
	private @Nullable UvMapping mapping;
	private int lightCoords;
	private int overlayCoords;
	private int color;
	private int packedColor;
	private long overlayLight;

	private ModelPartRenderer() {
	}

	public static void render(ModelPart part, PoseStack poseStack, VertexConsumer buffer, int lightCoords, int overlayCoords, int color) {
		RENDERERS.get().renderRoot(part, poseStack, buffer, lightCoords, overlayCoords, color);
	}

	private void renderRoot(ModelPart part, PoseStack poseStack, VertexConsumer buffer, int lightCoords, int overlayCoords, int color) {
		VertexConsumer target = buffer;
		UvMapping mapping = null;
		if (buffer instanceof SpriteCoordinateExpander expander) {
			target = expander.delegate();
			mapping = expander.mapping();
		}
		this.buffer = buffer;
		this.sink = target instanceof QuadVertexSink sink && EntityVertexLayout.accepts(sink) ? sink : null;
		this.mapping = mapping;
		this.lightCoords = lightCoords;
		this.overlayCoords = overlayCoords;
		this.color = color;
		this.packedColor = ARGB.toABGR(color);
		this.overlayLight = VertexWords.pair(overlayCoords, lightCoords);
		this.renderPart(part, poseStack);
		this.buffer = null;
		this.sink = null;
		this.mapping = null;
	}

	private void renderPart(ModelPart part, PoseStack poseStack) {
		if (!part.visible) {
			return;
		}
		ModelPartTree tree = (ModelPartTree) (Object) part;
		ModelPart.Cube[] cubes = tree.rikka$cubes();
		ModelPart[] children = tree.rikka$children();
		if (cubes.length == 0 && children.length == 0) {
			return;
		}
		poseStack.pushPose();
		applyPose(part, poseStack, this.rotation);
		if (!part.skipDraw) {
			this.compile(cubes, poseStack.last());
		}
		for (ModelPart child : children) {
			this.renderPart(child, poseStack);
		}
		poseStack.popPose();
	}

	private static void applyPose(ModelPart part, PoseStack poseStack, Quaternionf rotation) {
		poseStack.translate(part.x / PART_UNITS, part.y / PART_UNITS, part.z / PART_UNITS);
		if (part.xRot != 0.0F || part.yRot != 0.0F || part.zRot != 0.0F) {
			poseStack.rotate(rotation.rotationZYX(part.zRot, part.yRot, part.xRot));
		}
		if (part.xScale != 1.0F || part.yScale != 1.0F || part.zScale != 1.0F) {
			poseStack.scale(part.xScale, part.yScale, part.zScale);
		}
	}

	private void compile(ModelPart.Cube[] cubes, PoseStack.Pose pose) {
		VertexConsumer buffer = this.buffer;
		QuadVertexSink sink = this.sink;
		if (sink == null) {
			for (ModelPart.Cube cube : cubes) {
				cube.compile(pose, buffer, this.lightCoords, this.overlayCoords, this.color);
			}
			return;
		}
		if (cubes.length == 0) {
			return;
		}
		this.transform.load(pose);
		for (ModelPart.Cube cube : cubes) {
			CubeMesh mesh = ((CubeMeshHolder) cube).rikka$mesh();
			int vertexCount = mesh.vertexCount();
			if (vertexCount == 0) {
				continue;
			}
			long pointer = sink.rikka$reserveVertices(vertexCount);
			if (pointer == QuadVertexSink.NO_RESERVATION) {
				cube.compile(pose, buffer, this.lightCoords, this.overlayCoords, this.color);
				continue;
			}
			this.write(mesh, pointer);
		}
	}

	private void write(CubeMesh mesh, long pointer) {
		PartTransform transform = this.transform;
		int cornerCount = mesh.cornerCount();
		this.reserveCorners(cornerCount);
		long[] cornerXY = this.cornerXY;
		long[] cornerZColor = this.cornerZColor;
		transform.transformCorners(mesh.corners(), cornerCount, cornerXY, cornerZColor, this.packedColor);
		UvMapping mapping = this.mapping;
		long overlayLight = this.overlayLight;
		int[] polygonSizes = mesh.polygonSizes();
		int[] polygonDirections = mesh.polygonDirections();
		int[] vertexCorners = mesh.vertexCorners();
		long[] vertexUv = mesh.vertexUv();
		float[] vertexU = mesh.vertexU();
		float[] vertexV = mesh.vertexV();
		long address = pointer;
		int vertex = 0;
		for (int polygon = 0; polygon < polygonSizes.length; polygon++) {
			int normal = transform.normalWord(polygonDirections[polygon]);
			short normalXY = (short) normal;
			byte normalZ = (byte) (normal >>> 16);
			for (int end = vertex + polygonSizes[polygon]; vertex < end; vertex++) {
				int corner = vertexCorners[vertex];
				long uv = mapping == null ? vertexUv[vertex] : VertexWords.pair(mapping.getU(vertexU[vertex]), mapping.getV(vertexV[vertex]));
				MemoryUtil.memPutLong(address, cornerXY[corner]);
				MemoryUtil.memPutLong(address + EntityVertexLayout.Z_COLOR_OFFSET, cornerZColor[corner]);
				MemoryUtil.memPutLong(address + EntityVertexLayout.UV_OFFSET, uv);
				MemoryUtil.memPutLong(address + EntityVertexLayout.OVERLAY_LIGHT_OFFSET, overlayLight);
				MemoryUtil.memPutShort(address + EntityVertexLayout.NORMAL_OFFSET, normalXY);
				MemoryUtil.memPutByte(address + EntityVertexLayout.NORMAL_OFFSET + 2L, normalZ);
				address += EntityVertexLayout.VERTEX_SIZE;
			}
		}
	}

	private void reserveCorners(int count) {
		if (this.cornerXY.length < count) {
			this.cornerXY = new long[count];
			this.cornerZColor = new long[count];
		}
	}
}
