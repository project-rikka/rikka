package sh.rikka.client.render.entity;

import net.minecraft.client.model.geom.ModelPart;
import org.joml.Vector3fc;

public final class CubeMesh {
	public static final int NORMAL_DIRECTIONS = 6;
	private static final int POSITIVE_ZERO = Float.floatToRawIntBits(0.0F);

	private final float[] corners;
	private final int cornerCount;
	private final int[] polygonSizes;
	private final int[] polygonDirections;
	private final int[] vertexCorners;
	private final float[] vertexU;
	private final float[] vertexV;
	private final long[] vertexUv;

	private CubeMesh(float[] corners, int cornerCount, int[] polygonSizes, int[] polygonDirections, int[] vertexCorners, float[] vertexU, float[] vertexV) {
		this.corners = corners;
		this.cornerCount = cornerCount;
		this.polygonSizes = polygonSizes;
		this.polygonDirections = polygonDirections;
		this.vertexCorners = vertexCorners;
		this.vertexU = vertexU;
		this.vertexV = vertexV;
		this.vertexUv = new long[vertexU.length];
		for (int vertex = 0; vertex < vertexU.length; vertex++) {
			this.vertexUv[vertex] = VertexWords.pair(vertexU[vertex], vertexV[vertex]);
		}
	}

	public static CubeMesh of(ModelPart.Polygon[] polygons) {
		int[] polygonSizes = new int[polygons.length];
		int[] polygonDirections = new int[polygons.length];
		int vertexCount = 0;
		for (int polygonIndex = 0; polygonIndex < polygons.length; polygonIndex++) {
			polygonSizes[polygonIndex] = polygons[polygonIndex].vertices().length;
			polygonDirections[polygonIndex] = direction(polygons[polygonIndex].normal());
			vertexCount += polygonSizes[polygonIndex];
		}
		ModelPart.Vertex[] cornerVertices = new ModelPart.Vertex[vertexCount];
		int cornerCount = 0;
		int[] vertexCorners = new int[vertexCount];
		float[] vertexU = new float[vertexCount];
		float[] vertexV = new float[vertexCount];
		int index = 0;
		for (ModelPart.Polygon polygon : polygons) {
			for (ModelPart.Vertex vertex : polygon.vertices()) {
				int corner = indexOf(cornerVertices, cornerCount, vertex);
				if (corner == cornerCount) {
					cornerVertices[cornerCount++] = vertex;
				}
				vertexCorners[index] = corner;
				vertexU[index] = vertex.u();
				vertexV[index] = vertex.v();
				index++;
			}
		}
		float[] corners = new float[cornerCount * 3];
		for (int corner = 0; corner < cornerCount; corner++) {
			ModelPart.Vertex vertex = cornerVertices[corner];
			corners[corner * 3] = vertex.worldX();
			corners[corner * 3 + 1] = vertex.worldY();
			corners[corner * 3 + 2] = vertex.worldZ();
		}
		return new CubeMesh(corners, cornerCount, polygonSizes, polygonDirections, vertexCorners, vertexU, vertexV);
	}

	public static int normalDirection(int axis, boolean negative) {
		return axis * 2 + (negative ? 1 : 0);
	}

	private static int direction(Vector3fc normal) {
		float[] components = {normal.x(), normal.y(), normal.z()};
		for (int axis = 0; axis < 3; axis++) {
			if (isAxisNormal(components, axis)) {
				return normalDirection(axis, components[axis] < 0.0F);
			}
		}
		throw new IllegalStateException("model polygon normal is not a unit axis: " + normal);
	}

	private static boolean isAxisNormal(float[] components, int axis) {
		for (int other = 0; other < 3; other++) {
			boolean matches = other == axis
				? components[other] == 1.0F || components[other] == -1.0F
				: Float.floatToRawIntBits(components[other]) == POSITIVE_ZERO;
			if (!matches) {
				return false;
			}
		}
		return true;
	}

	private static int indexOf(ModelPart.Vertex[] cornerVertices, int cornerCount, ModelPart.Vertex vertex) {
		for (int corner = 0; corner < cornerCount; corner++) {
			ModelPart.Vertex candidate = cornerVertices[corner];
			if (Float.floatToRawIntBits(candidate.x()) == Float.floatToRawIntBits(vertex.x())
				&& Float.floatToRawIntBits(candidate.y()) == Float.floatToRawIntBits(vertex.y())
				&& Float.floatToRawIntBits(candidate.z()) == Float.floatToRawIntBits(vertex.z())) {
				return corner;
			}
		}
		return cornerCount;
	}

	public float[] corners() {
		return this.corners;
	}

	public int cornerCount() {
		return this.cornerCount;
	}

	public int[] polygonSizes() {
		return this.polygonSizes;
	}

	public int[] polygonDirections() {
		return this.polygonDirections;
	}

	public int[] vertexCorners() {
		return this.vertexCorners;
	}

	public float[] vertexU() {
		return this.vertexU;
	}

	public float[] vertexV() {
		return this.vertexV;
	}

	public long[] vertexUv() {
		return this.vertexUv;
	}

	public int vertexCount() {
		return this.vertexCorners.length;
	}
}
