package sh.rikka.client.mods.visual;

import net.minecraft.client.Minecraft;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.network.protocol.game.ClientboundAnimatePacket;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.HitEvent;
import sh.rikka.client.events.PacketEvent;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.IntSetting;

public final class ParticlesMod extends Mod {
	private final EnumSetting<ParticleTrigger> criticals = choice("criticals", ParticleTrigger.NATURAL);
	private final IntSetting criticalsAmount = integer("criticals_amount", 1, 1, 10).visibleWhen(() -> !this.criticals.is(ParticleTrigger.OFF));
	private final EnumSetting<ParticleTrigger> sharpness = choice("sharpness", ParticleTrigger.NATURAL);
	private final IntSetting sharpnessAmount = integer("sharpness_amount", 1, 1, 10).visibleWhen(() -> !this.sharpness.is(ParticleTrigger.OFF));
	private final ParticleChannel criticalChannel = new ParticleChannel(this.criticals, this.criticalsAmount, ParticleTypes.CRIT);
	private final ParticleChannel sharpnessChannel = new ParticleChannel(this.sharpness, this.sharpnessAmount, ParticleTypes.ENCHANTED_HIT);

	public ParticlesMod() {
		super("particles", Category.VISUAL, "flare");
	}

	@EventHandler
	private void onReceive(PacketEvent.Receive event) {
		if (!(event.packet() instanceof ClientboundAnimatePacket animate)) {
			return;
		}
		int entityId = animate.getId();
		int action = animate.getAction();
		if (action == ClientboundAnimatePacket.CRITICAL_HIT) {
			Minecraft.getInstance().execute(() -> this.criticalChannel.emit(entityId, ParticleTrigger.NATURAL));
		} else if (action == ClientboundAnimatePacket.MAGIC_CRITICAL_HIT) {
			Minecraft.getInstance().execute(() -> this.sharpnessChannel.emit(entityId, ParticleTrigger.NATURAL));
		}
	}

	@EventHandler
	private void onHit(HitEvent event) {
		this.criticalChannel.emit(event.targetId(), ParticleTrigger.ALWAYS);
		this.sharpnessChannel.emit(event.targetId(), ParticleTrigger.ALWAYS);
	}
}
