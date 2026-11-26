package sh.rikka.client.render.entity;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.QuadInstance;
import net.minecraft.client.model.geom.builders.UVPair;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.util.ARGB;
import org.joml.Math;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;
import org.joml.Vector3fc;
import org.lwjgl.system.MemoryUtil;

public final class BakedQuadWriter {
	private BakedQuadWriter() {
	}

	public static boolean write(QuadVertexSink sink, PoseStack.Pose pose, BakedQuad quad, QuadInstance instance) {
		if (!EntityVertexLayout.accepts(sink)) {
			return false;
		}
		long address = sink.rikka$reserveVertices(BakedQuad.VERTEX_COUNT);
		if (address == QuadVertexSink.NO_RESERVATION) {
			return false;
		}
		Matrix4f matrix = pose.pose();
		int properties = matrix.properties();
		Vector3fc unit = quad.direction().getUnitVec3f();
		int normal = NormalWords.transform(pose, unit.x(), unit.y(), unit.z());
		short normalXY = (short) normal;
		byte normalZ = (byte) (normal >>> 16);
		int lightEmission = quad.materialInfo().lightEmission();
		int overlayCoords = instance.overlayCoords();
		for (int vertex = 0; vertex < BakedQuad.VERTEX_COUNT; vertex++) {
			Vector3fc position = quad.position(vertex);
			long packedUv = quad.packedUV(vertex);
			float x = position.x();
			float y = position.y();
			float z = position.z();
			float transformedX;
			float transformedY;
			float transformedZ;
			if ((properties & Matrix4fc.PROPERTY_IDENTITY) != 0) {
				transformedX = x;
				transformedY = y;
				transformedZ = z;
			} else if ((properties & Matrix4fc.PROPERTY_TRANSLATION) != 0) {
				transformedX = x + matrix.m30();
				transformedY = y + matrix.m31();
				transformedZ = z + matrix.m32();
			} else {
				transformedX = Math.fma(matrix.m00(), x, Math.fma(matrix.m10(), y, Math.fma(matrix.m20(), z, matrix.m30())));
				transformedY = Math.fma(matrix.m01(), x, Math.fma(matrix.m11(), y, Math.fma(matrix.m21(), z, matrix.m31())));
				transformedZ = Math.fma(matrix.m02(), x, Math.fma(matrix.m12(), y, Math.fma(matrix.m22(), z, matrix.m32())));
			}
			MemoryUtil.memPutLong(address, VertexWords.pair(transformedX, transformedY));
			MemoryUtil.memPutLong(address + EntityVertexLayout.Z_COLOR_OFFSET, VertexWords.pair(transformedZ, ARGB.toABGR(instance.getColor(vertex))));
			MemoryUtil.memPutLong(address + EntityVertexLayout.UV_OFFSET, VertexWords.pair(UVPair.unpackU(packedUv), UVPair.unpackV(packedUv)));
			MemoryUtil.memPutLong(
				address + EntityVertexLayout.OVERLAY_LIGHT_OFFSET,
				VertexWords.pair(overlayCoords, instance.getLightCoordsWithEmission(vertex, lightEmission))
			);
			MemoryUtil.memPutShort(address + EntityVertexLayout.NORMAL_OFFSET, normalXY);
			MemoryUtil.memPutByte(address + EntityVertexLayout.NORMAL_OFFSET + 2L, normalZ);
			address += EntityVertexLayout.VERTEX_SIZE;
		}
		return true;
	}
}
