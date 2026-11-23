package sh.rikka.client.mods.visual;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.projectile.Projectile;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.EnumSetting;

public final class ProjectileTrailMod extends Mod {
	private static final double PARTICLE_RANGE_SQR = 32.0 * 32.0;
	private static final double MIN_TRAVEL_SQR = 0.01;
	private static final double SPACING = 0.4;
	private static final int MAX_PARTICLES_PER_TICK = 8;

	private final EnumSetting<ProjectileTrailEffect> effect = choice("effect", ProjectileTrailEffect.HEARTS);
	private final BoolSetting ownOnly = bool("own_only", false);

	public ProjectileTrailMod() {
		super("projectile_trail", Category.VISUAL, "trail_length");
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		ClientLevel level = minecraft.level;
		if (player == null || level == null || minecraft.isPaused() || !level.tickRateManager().runsNormally()) {
			return;
		}
		ParticleOptions particle = this.effect.get().particle();
		for (Entity entity : level.entitiesForRendering()) {
			if (entity instanceof Projectile projectile && shows(projectile, player)) {
				trail(level, projectile, particle);
			}
		}
	}

	private boolean shows(Projectile projectile, LocalPlayer player) {
		return !projectile.isRemoved()
			&& projectile.distanceToSqr(player) <= PARTICLE_RANGE_SQR
			&& (!this.ownOnly.get() || projectile.getOwner() == player);
	}

	private static void trail(ClientLevel level, Projectile projectile, ParticleOptions particle) {
		double dx = projectile.getX() - projectile.xo;
		double dy = projectile.getY() - projectile.yo;
		double dz = projectile.getZ() - projectile.zo;
		double travelSqr = dx * dx + dy * dy + dz * dz;
		if (travelSqr < MIN_TRAVEL_SQR) {
			return;
		}
		int count = Mth.clamp((int) Math.ceil(Math.sqrt(travelSqr) / SPACING), 1, MAX_PARTICLES_PER_TICK);
		for (int step = 1; step <= count; step++) {
			double progress = (double) step / count;
			level.addParticle(particle, projectile.xo + dx * progress, projectile.yo + dy * progress, projectile.zo + dz * progress, 0.0, 0.0, 0.0);
		}
	}
}
