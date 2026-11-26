package sh.rikka.bench;

public enum Preset {
	FAST(8, 6, 1, 0, false, "fast", 1, 2, false, 0.75, 32, false, false, 5, 1, 0),
	FANCY(16, 12, 2, 1, true, "true", 0, 4, true, 1.0, 64, true, false, 10, 1, 1),
	FABULOUS(32, 12, 2, 1, true, "true", 0, 4, true, 1.25, 128, true, true, 10, 2, 2);

	final int renderDistance;
	final int simulationDistance;
	final int biomeBlendRadius;
	final int prioritizeChunkUpdates;
	final boolean ambientOcclusion;
	final String clouds;
	final int particles;
	final int mipmapLevels;
	final boolean entityShadows;
	final double entityDistanceScaling;
	final int cloudRange;
	final boolean cutoutLeaves;
	final boolean improvedTransparency;
	final int weatherRadius;
	final int maxAnisotropyBit;
	final int textureFiltering;

	Preset(
		int renderDistance,
		int simulationDistance,
		int biomeBlendRadius,
		int prioritizeChunkUpdates,
		boolean ambientOcclusion,
		String clouds,
		int particles,
		int mipmapLevels,
		boolean entityShadows,
		double entityDistanceScaling,
		int cloudRange,
		boolean cutoutLeaves,
		boolean improvedTransparency,
		int weatherRadius,
		int maxAnisotropyBit,
		int textureFiltering
	) {
		this.renderDistance = renderDistance;
		this.simulationDistance = simulationDistance;
		this.biomeBlendRadius = biomeBlendRadius;
		this.prioritizeChunkUpdates = prioritizeChunkUpdates;
		this.ambientOcclusion = ambientOcclusion;
		this.clouds = clouds;
		this.particles = particles;
		this.mipmapLevels = mipmapLevels;
		this.entityShadows = entityShadows;
		this.entityDistanceScaling = entityDistanceScaling;
		this.cloudRange = cloudRange;
		this.cutoutLeaves = cutoutLeaves;
		this.improvedTransparency = improvedTransparency;
		this.weatherRadius = weatherRadius;
		this.maxAnisotropyBit = maxAnisotropyBit;
		this.textureFiltering = textureFiltering;
	}
}
