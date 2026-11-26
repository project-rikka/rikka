package sh.rikka.client.render.skin;

import com.mojang.blaze3d.vertex.PoseStack;
import java.util.Arrays;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.util.ARGB;
import org.joml.Matrix3f;
import org.joml.Matrix4f;
import org.lwjgl.system.MemoryUtil;
import sh.rikka.client.mixin.entity.PoseAccessor;
import sh.rikka.client.render.entity.ModelPartRenderer;
import sh.rikka.client.render.entity.ModelPartTree;

public final class SkinWriter {
	private static final int WHITE = -1;
	private static final float COLOR_RANGE = 255.0F;
	private static final int POSE_FLOATS = 12;
	private static final int NORMAL_FLOATS = 9;
	private static final int INITIAL_DEPTH = 16;

	private final PartRotations rotations = new PartRotations();
	private float[] poses = new float[POSE_FLOATS * INITIAL_DEPTH];
	private float[] normals = new float[NORMAL_FLOATS * INITIAL_DEPTH];
	private boolean[] trusted = new boolean[INITIAL_DEPTH];
	private long cursor;

	public static boolean isAffine(PoseStack.Pose pose) {
		Matrix4f matrix = pose.pose();
		return matrix.m03() == 0.0F && matrix.m13() == 0.0F && matrix.m23() == 0.0F && matrix.m33() == 1.0F;
	}

	public void writeModel(
		SkinFrame frame,
		SkinGroup group,
		RenderType renderType,
		ModelSkin skin,
		ModelPart root,
		PoseStack.Pose pose,
		int lightCoords,
		int overlayCoords,
		int color,
		UvTransform uv
	) {
		SkinBatch batch = group.rikka$skinBatch(frame, renderType);
		long rows = frame.append(batch, skin.baseVertex(), skin.indexCount(), skin.rows(), ARGB.toABGR(color), overlayCoords, lightCoords, uv);
		MemoryUtil.memSet(rows, 0, (long) skin.boneCount() * SkinLayout.BONE_BYTES);
		this.cursor = rows;
		load(pose);
		writePart(root, 0);
	}

	public void writeItem(
		SkinFrame frame, SkinGroup group, ItemSkin skin, boolean glint, PoseStack.Pose pose, int lightCoords, int overlayCoords, int[] tintLayers
	) {
		load(pose);
		for (ItemSkin.Part part : skin.parts()) {
			SkinBatch batch = group.rikka$skinBatch(frame, glint ? part.glintRenderType() : part.renderType());
			long rows = frame.append(batch, part.baseVertex(), part.indexCount(), skin.rows(), WHITE, overlayCoords, lightCoords, UvTransform.identity());
			writeBone(rows, 0);
			long tints = rows + SkinLayout.BONE_BYTES;
			for (int tint = 0; tint < skin.tintCount(); tint++) {
				writeColor(tints + (long) tint * SkinLayout.ROW_BYTES, tint < tintLayers.length ? tintLayers[tint] : WHITE);
			}
		}
	}

	private void load(PoseStack.Pose pose) {
		Matrix4f matrix = pose.pose();
		float[] poses = this.poses;
		poses[0] = matrix.m00();
		poses[1] = matrix.m01();
		poses[2] = matrix.m02();
		poses[3] = matrix.m10();
		poses[4] = matrix.m11();
		poses[5] = matrix.m12();
		poses[6] = matrix.m20();
		poses[7] = matrix.m21();
		poses[8] = matrix.m22();
		poses[9] = matrix.m30();
		poses[10] = matrix.m31();
		poses[11] = matrix.m32();
		Matrix3f normal = pose.normal();
		float[] normals = this.normals;
		normals[0] = normal.m00();
		normals[1] = normal.m01();
		normals[2] = normal.m02();
		normals[3] = normal.m10();
		normals[4] = normal.m11();
		normals[5] = normal.m12();
		normals[6] = normal.m20();
		normals[7] = normal.m21();
		normals[8] = normal.m22();
		this.trusted[0] = ((PoseAccessor) (Object) pose).rikka$trustedNormals();
	}

	private void writePart(ModelPart part, int depth) {
		ModelPartTree tree = (ModelPartTree) (Object) part;
		if (!part.visible) {
			this.cursor += (long) tree.rikka$boneCount() * SkinLayout.BONE_BYTES;
			return;
		}
		ModelPart[] children = tree.rikka$children();
		boolean bone = tree.rikka$cubes().length != 0;
		if (!bone && children.length == 0) {
			return;
		}
		int level = depth + 1;
		pose(part, level);
		if (bone) {
			if (!part.skipDraw) {
				writeBone(this.cursor, level);
			}
			this.cursor += SkinLayout.BONE_BYTES;
		}
		for (ModelPart child : children) {
			writePart(child, level);
		}
	}

	private void pose(ModelPart part, int level) {
		if (level == this.trusted.length) {
			this.poses = Arrays.copyOf(this.poses, this.poses.length * 2);
			this.normals = Arrays.copyOf(this.normals, this.normals.length * 2);
			this.trusted = Arrays.copyOf(this.trusted, this.trusted.length * 2);
		}
		float[] poses = this.poses;
		float[] normals = this.normals;
		int pose = level * POSE_FLOATS;
		int parentPose = pose - POSE_FLOATS;
		int normal = level * NORMAL_FLOATS;
		int parentNormal = normal - NORMAL_FLOATS;
		float m00 = poses[parentPose];
		float m01 = poses[parentPose + 1];
		float m02 = poses[parentPose + 2];
		float m10 = poses[parentPose + 3];
		float m11 = poses[parentPose + 4];
		float m12 = poses[parentPose + 5];
		float m20 = poses[parentPose + 6];
		float m21 = poses[parentPose + 7];
		float m22 = poses[parentPose + 8];
		float n00 = normals[parentNormal];
		float n01 = normals[parentNormal + 1];
		float n02 = normals[parentNormal + 2];
		float n10 = normals[parentNormal + 3];
		float n11 = normals[parentNormal + 4];
		float n12 = normals[parentNormal + 5];
		float n20 = normals[parentNormal + 6];
		float n21 = normals[parentNormal + 7];
		float n22 = normals[parentNormal + 8];
		boolean trusted = this.trusted[level - 1];
		float x = part.x / ModelPartRenderer.PART_UNITS;
		float y = part.y / ModelPartRenderer.PART_UNITS;
		float z = part.z / ModelPartRenderer.PART_UNITS;
		poses[pose + 9] = Math.fma(m00, x, Math.fma(m10, y, Math.fma(m20, z, poses[parentPose + 9])));
		poses[pose + 10] = Math.fma(m01, x, Math.fma(m11, y, Math.fma(m21, z, poses[parentPose + 10])));
		poses[pose + 11] = Math.fma(m02, x, Math.fma(m12, y, Math.fma(m22, z, poses[parentPose + 11])));
		if (part.xRot != 0.0F || part.yRot != 0.0F || part.zRot != 0.0F) {
			float[] rotation = this.rotations.matrices();
			int r = this.rotations.find(part.xRot, part.yRot, part.zRot);
			float r00 = rotation[r];
			float r01 = rotation[r + 1];
			float r02 = rotation[r + 2];
			float r10 = rotation[r + 3];
			float r11 = rotation[r + 4];
			float r12 = rotation[r + 5];
			float r20 = rotation[r + 6];
			float r21 = rotation[r + 7];
			float r22 = rotation[r + 8];
			float p00 = Math.fma(m00, r00, Math.fma(m10, r01, m20 * r02));
			float p01 = Math.fma(m01, r00, Math.fma(m11, r01, m21 * r02));
			float p02 = Math.fma(m02, r00, Math.fma(m12, r01, m22 * r02));
			float p10 = Math.fma(m00, r10, Math.fma(m10, r11, m20 * r12));
			float p11 = Math.fma(m01, r10, Math.fma(m11, r11, m21 * r12));
			float p12 = Math.fma(m02, r10, Math.fma(m12, r11, m22 * r12));
			m20 = Math.fma(m00, r20, Math.fma(m10, r21, m20 * r22));
			m21 = Math.fma(m01, r20, Math.fma(m11, r21, m21 * r22));
			m22 = Math.fma(m02, r20, Math.fma(m12, r21, m22 * r22));
			m00 = p00;
			m01 = p01;
			m02 = p02;
			m10 = p10;
			m11 = p11;
			m12 = p12;
			float q00 = Math.fma(n00, r00, Math.fma(n10, r01, n20 * r02));
			float q01 = Math.fma(n01, r00, Math.fma(n11, r01, n21 * r02));
			float q02 = Math.fma(n02, r00, Math.fma(n12, r01, n22 * r02));
			float q10 = Math.fma(n00, r10, Math.fma(n10, r11, n20 * r12));
			float q11 = Math.fma(n01, r10, Math.fma(n11, r11, n21 * r12));
			float q12 = Math.fma(n02, r10, Math.fma(n12, r11, n22 * r12));
			n20 = Math.fma(n00, r20, Math.fma(n10, r21, n20 * r22));
			n21 = Math.fma(n01, r20, Math.fma(n11, r21, n21 * r22));
			n22 = Math.fma(n02, r20, Math.fma(n12, r21, n22 * r22));
			n00 = q00;
			n01 = q01;
			n02 = q02;
			n10 = q10;
			n11 = q11;
			n12 = q12;
		}
		float xScale = part.xScale;
		float yScale = part.yScale;
		float zScale = part.zScale;
		if (xScale != 1.0F || yScale != 1.0F || zScale != 1.0F) {
			m00 *= xScale;
			m01 *= xScale;
			m02 *= xScale;
			m10 *= yScale;
			m11 *= yScale;
			m12 *= yScale;
			m20 *= zScale;
			m21 *= zScale;
			m22 *= zScale;
			float xNormal;
			float yNormal;
			float zNormal;
			if (Math.abs(xScale) == Math.abs(yScale) && Math.abs(yScale) == Math.abs(zScale)) {
				xNormal = Math.signum(xScale);
				yNormal = Math.signum(yScale);
				zNormal = Math.signum(zScale);
			} else {
				xNormal = 1.0F / xScale;
				yNormal = 1.0F / yScale;
				zNormal = 1.0F / zScale;
				trusted = false;
			}
			n00 *= xNormal;
			n01 *= xNormal;
			n02 *= xNormal;
			n10 *= yNormal;
			n11 *= yNormal;
			n12 *= yNormal;
			n20 *= zNormal;
			n21 *= zNormal;
			n22 *= zNormal;
		}
		poses[pose] = m00;
		poses[pose + 1] = m01;
		poses[pose + 2] = m02;
		poses[pose + 3] = m10;
		poses[pose + 4] = m11;
		poses[pose + 5] = m12;
		poses[pose + 6] = m20;
		poses[pose + 7] = m21;
		poses[pose + 8] = m22;
		normals[normal] = n00;
		normals[normal + 1] = n01;
		normals[normal + 2] = n02;
		normals[normal + 3] = n10;
		normals[normal + 4] = n11;
		normals[normal + 5] = n12;
		normals[normal + 6] = n20;
		normals[normal + 7] = n21;
		normals[normal + 8] = n22;
		this.trusted[level] = trusted;
	}

	private void writeBone(long address, int level) {
		float[] poses = this.poses;
		int pose = level * POSE_FLOATS;
		MemoryUtil.memPutFloat(address, poses[pose]);
		MemoryUtil.memPutFloat(address + 4L, poses[pose + 3]);
		MemoryUtil.memPutFloat(address + 8L, poses[pose + 6]);
		MemoryUtil.memPutFloat(address + 12L, poses[pose + 9]);
		MemoryUtil.memPutFloat(address + 16L, poses[pose + 1]);
		MemoryUtil.memPutFloat(address + 20L, poses[pose + 4]);
		MemoryUtil.memPutFloat(address + 24L, poses[pose + 7]);
		MemoryUtil.memPutFloat(address + 28L, poses[pose + 10]);
		MemoryUtil.memPutFloat(address + 32L, poses[pose + 2]);
		MemoryUtil.memPutFloat(address + 36L, poses[pose + 5]);
		MemoryUtil.memPutFloat(address + 40L, poses[pose + 8]);
		MemoryUtil.memPutFloat(address + 44L, poses[pose + 11]);
		float[] normals = this.normals;
		int normal = level * NORMAL_FLOATS;
		long normalAddress = address + SkinLayout.NORMAL_ROW_OFFSET;
		MemoryUtil.memPutFloat(normalAddress, normals[normal]);
		MemoryUtil.memPutFloat(normalAddress + 4L, normals[normal + 3]);
		MemoryUtil.memPutFloat(normalAddress + 8L, normals[normal + 6]);
		MemoryUtil.memPutFloat(normalAddress + 12L, this.trusted[level] ? 0.0F : 1.0F);
		MemoryUtil.memPutFloat(normalAddress + 16L, normals[normal + 1]);
		MemoryUtil.memPutFloat(normalAddress + 20L, normals[normal + 4]);
		MemoryUtil.memPutFloat(normalAddress + 24L, normals[normal + 7]);
		MemoryUtil.memPutFloat(normalAddress + 28L, 0.0F);
		MemoryUtil.memPutFloat(normalAddress + 32L, normals[normal + 2]);
		MemoryUtil.memPutFloat(normalAddress + 36L, normals[normal + 5]);
		MemoryUtil.memPutFloat(normalAddress + 40L, normals[normal + 8]);
		MemoryUtil.memPutFloat(normalAddress + 44L, 0.0F);
	}

	private static void writeColor(long address, int argb) {
		MemoryUtil.memPutFloat(address, ARGB.red(argb) / COLOR_RANGE);
		MemoryUtil.memPutFloat(address + 4L, ARGB.green(argb) / COLOR_RANGE);
		MemoryUtil.memPutFloat(address + 8L, ARGB.blue(argb) / COLOR_RANGE);
		MemoryUtil.memPutFloat(address + 12L, ARGB.alpha(argb) / COLOR_RANGE);
	}
}
