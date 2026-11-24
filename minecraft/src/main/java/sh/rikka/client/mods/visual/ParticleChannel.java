package sh.rikka.client.mods.visual;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.world.entity.Entity;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.IntSetting;

record ParticleChannel(EnumSetting<ParticleTrigger> trigger, IntSetting amount, ParticleOptions particle) {
	void emit(int entityId, ParticleTrigger cause) {
		if (!this.trigger.is(cause)) {
			return;
		}
		ClientLevel level = Minecraft.getInstance().level;
		Entity entity = level == null ? null : level.getEntity(entityId);
		if (entity != null) {
			ParticleBursts.emit(entity, this.particle, this.amount.get());
		}
	}
}
