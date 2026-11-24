package sh.rikka.client.mods.visual;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.particles.BlockParticleOption;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.sounds.SoundEvents;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.block.Blocks;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.HitEvent;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.IntSetting;

public final class BloodParticleMod extends Mod {
	private static final float SOUND_VOLUME = 4.0F;
	private static final float SOUND_PITCH = 1.2F;

	private final IntSetting amount = integer("amount", 2, 1, 10);
	private final BoolSetting sound = bool("sound", true);

	public BloodParticleMod() {
		super("blood_particle", Category.VISUAL, "bloodtype");
	}

	@EventHandler
	private void onHit(HitEvent event) {
		ClientLevel level = Minecraft.getInstance().level;
		if (level == null || !(level.getEntity(event.targetId()) instanceof LivingEntity target)) {
			return;
		}
		BlockParticleOption blood = new BlockParticleOption(ParticleTypes.BLOCK, Blocks.REDSTONE_BLOCK.defaultBlockState());
		ParticleBursts.emit(target, blood, this.amount.get());
		if (this.sound.get()) {
			level.playLocalSound(target.getX(), target.getY(), target.getZ(), SoundEvents.STONE_BREAK, SoundSource.BLOCKS, SOUND_VOLUME, SOUND_PITCH, false);
		}
	}
}
