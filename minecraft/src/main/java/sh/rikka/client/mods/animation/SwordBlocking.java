package sh.rikka.client.mods.animation;

import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;

final class SwordBlocking {
	private final OldAnimationsMod mod;
	private boolean spent;
	private boolean engaged;

	SwordBlocking(OldAnimationsMod mod) {
		this.mod = mod;
	}

	void spend() {
		this.spent = true;
	}

	void reset() {
		this.spent = false;
		this.engaged = false;
	}

	void tick(Minecraft minecraft) {
		if (!HeldInput.held(minecraft, minecraft.options.keyUse)) {
			reset();
			return;
		}
		LocalPlayer player = minecraft.player;
		this.engaged = !this.spent && !player.isSpectator() && this.mod.blocksWith(player.getMainHandItem());
	}

	boolean isBlocking(LocalPlayer player) {
		return this.engaged && !player.isUsingItem() && this.mod.blocksWith(player.getMainHandItem());
	}
}
