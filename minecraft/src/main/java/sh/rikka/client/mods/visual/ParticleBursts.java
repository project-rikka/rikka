package sh.rikka.client.mods.visual;

import net.minecraft.client.Minecraft;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.world.entity.Entity;

final class ParticleBursts {
	private ParticleBursts() {
	}

	static void emit(Entity entity, ParticleOptions particle, int amount) {
		Minecraft minecraft = Minecraft.getInstance();
		for (int burst = 0; burst < amount; burst++) {
			minecraft.particleEngine.createTrackingEmitter(entity, particle);
		}
	}
}
