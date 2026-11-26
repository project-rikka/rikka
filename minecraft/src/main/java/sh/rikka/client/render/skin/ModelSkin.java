package sh.rikka.client.render.skin;

import net.minecraft.client.model.geom.ModelPart;
import sh.rikka.client.render.entity.ModelPartTree;

public record ModelSkin(int baseVertex, int indexCount, int boneCount) {
	private static final ModelSkin UNSUPPORTED = new ModelSkin(0, 0, 0);

	public boolean isSupported() {
		return this.indexCount != 0;
	}

	public int rows() {
		return this.boneCount * SkinLayout.BONE_ROWS;
	}

	static ModelSkin build(ModelPart root, SkinMeshPool pool) {
		int boneCount = ((ModelPartTree) (Object) root).rikka$boneCount();
		int quadCount = countQuads(root);
		if (quadCount <= 0 || boneCount * SkinLayout.BONE_ROWS > SkinLayout.WINDOW_ROWS) {
			return UNSUPPORTED;
		}
		int baseVertex = pool.allocate(quadCount * SkinLayout.VERTICES_PER_QUAD);
		writePart(root, new SkinVertexWriter(pool.address(baseVertex)), 0);
		return new ModelSkin(baseVertex, quadCount * SkinLayout.INDICES_PER_QUAD, boneCount);
	}

	private static int countQuads(ModelPart part) {
		ModelPartTree tree = (ModelPartTree) (Object) part;
		int quads = 0;
		for (ModelPart.Cube cube : tree.rikka$cubes()) {
			for (ModelPart.Polygon polygon : cube.polygons) {
				if (polygon.vertices().length != SkinLayout.VERTICES_PER_QUAD) {
					return Integer.MIN_VALUE;
				}
				quads++;
			}
		}
		for (ModelPart child : tree.rikka$children()) {
			quads += countQuads(child);
			if (quads < 0) {
				return Integer.MIN_VALUE;
			}
		}
		return quads;
	}

	private static int writePart(ModelPart part, SkinVertexWriter writer, int bone) {
		ModelPartTree tree = (ModelPartTree) (Object) part;
		ModelPart.Cube[] cubes = tree.rikka$cubes();
		int next = bone;
		if (cubes.length != 0) {
			int boneRow = bone * SkinLayout.BONE_ROWS;
			for (ModelPart.Cube cube : cubes) {
				for (ModelPart.Polygon polygon : cube.polygons) {
					for (ModelPart.Vertex vertex : polygon.vertices()) {
						writer.put(vertex.worldX(), vertex.worldY(), vertex.worldZ(), vertex.u(), vertex.v(), polygon.normal(), boneRow, SkinLayout.NO_TINT, 0);
					}
				}
			}
			next++;
		}
		for (ModelPart child : tree.rikka$children()) {
			next = writePart(child, writer, next);
		}
		return next;
	}
}
