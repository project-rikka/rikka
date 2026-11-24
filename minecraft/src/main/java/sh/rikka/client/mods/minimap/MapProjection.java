package sh.rikka.client.mods.minimap;

import net.minecraft.util.Mth;
import sh.rikka.client.render.GuiMesh;

record MapProjection(
	float centerX,
	float centerY,
	float halfWidth,
	float halfHeight,
	MapShape shape,
	float zoom,
	float rotation,
	double playerX,
	double playerZ
) implements GuiMesh.UvMap {
	float screenX(double worldX, double worldZ) {
		double dx = worldX - this.playerX;
		double dz = worldZ - this.playerZ;
		return this.centerX + (float) (Mth.cos(this.rotation) * dx + Mth.sin(this.rotation) * dz) * this.zoom;
	}

	float screenY(double worldX, double worldZ) {
		double dx = worldX - this.playerX;
		double dz = worldZ - this.playerZ;
		return this.centerY + (float) (-Mth.sin(this.rotation) * dx + Mth.cos(this.rotation) * dz) * this.zoom;
	}

	boolean contains(float x, float y, float margin) {
		return this.shape.contains(x - this.centerX, y - this.centerY, this.halfWidth - margin, this.halfHeight - margin);
	}

	float rimDistance(float directionX, float directionY) {
		return this.shape.rimDistance(directionX, directionY, this.halfWidth, this.halfHeight);
	}

	@Override
	public float u(float x, float y) {
		float sx = x - this.centerX;
		float sy = y - this.centerY;
		double dx = (Mth.cos(this.rotation) * sx - Mth.sin(this.rotation) * sy) / this.zoom;
		return (float) ((wrapped(this.playerX) + dx) / TerrainAtlas.SIZE);
	}

	@Override
	public float v(float x, float y) {
		float sx = x - this.centerX;
		float sy = y - this.centerY;
		double dz = (Mth.sin(this.rotation) * sx + Mth.cos(this.rotation) * sy) / this.zoom;
		return (float) ((wrapped(this.playerZ) + dz) / TerrainAtlas.SIZE);
	}

	private static double wrapped(double coordinate) {
		return coordinate - Math.floor(coordinate / TerrainAtlas.SIZE) * TerrainAtlas.SIZE;
	}
}
