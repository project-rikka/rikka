package sh.rikka.client.mods.motionblur;

public enum BlurAlgorithm {
	VELOCITY,
	FRAME_BLENDING,
	HYBRID,
	ACCUMULATION_MAX,
	ACCUMULATION_MIX;

	boolean usesVelocity() {
		return this == VELOCITY || this == HYBRID;
	}

	boolean blendsFrames() {
		return this == FRAME_BLENDING || this == HYBRID;
	}

	boolean accumulates() {
		return this == ACCUMULATION_MAX || this == ACCUMULATION_MIX;
	}
}
