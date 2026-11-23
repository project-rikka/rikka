package sh.rikka.client.mods.animation;

import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.ShieldItem;
import net.minecraft.world.item.SwingAnimationType;
import org.jspecify.annotations.Nullable;

final class HeldItems {
	private HeldItems() {
	}

	static boolean isStabbing(LivingEntity.@Nullable SwingDescription swing) {
		return swing != null && swing.animation().type() == SwingAnimationType.STAB;
	}

	static boolean isShield(ItemStack stack) {
		return stack.getItem() instanceof ShieldItem;
	}
}
