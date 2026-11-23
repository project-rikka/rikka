package sh.rikka.client.mods.visual;

import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.core.particles.ParticleTypes;

public enum ProjectileTrailEffect {
	BLACK_SMOKE(ParticleTypes.SMOKE),
	WHITE_SMOKE(ParticleTypes.WHITE_SMOKE),
	FIRE(ParticleTypes.FLAME),
	SPARK(ParticleTypes.FIREWORK),
	MAGIC(ParticleTypes.WITCH),
	SWIRL(ParticleTypes.ENCHANT),
	GREEN_STAR(ParticleTypes.HAPPY_VILLAGER),
	HEARTS(ParticleTypes.HEART),
	MUSIC_NOTE(ParticleTypes.NOTE),
	SLIME(ParticleTypes.ITEM_SLIME);

	private final ParticleOptions particle;

	ProjectileTrailEffect(ParticleOptions particle) {
		this.particle = particle;
	}

	ParticleOptions particle() {
		return this.particle;
	}
}
