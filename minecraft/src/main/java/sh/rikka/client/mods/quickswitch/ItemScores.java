package sh.rikka.client.mods.quickswitch;

import net.minecraft.core.BlockPos;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.component.ItemAttributeModifiers;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.EnchantmentHelper;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.FallingBlock;
import net.minecraft.world.level.block.state.BlockState;

record ItemScores(RegistryAccess registries) {
	static final double UNMATCHED = -1.0;

	private static final double BASE_ATTACK_DAMAGE = 1.0;
	private static final double SHARPNESS_PER_LEVEL = 0.5;
	private static final double WEAR_WEIGHT = 0.01;

	double attackDamage(ItemStack stack, ResourceKey<Enchantment> sharpness) {
		ItemAttributeModifiers modifiers = stack.getOrDefault(DataComponents.ATTRIBUTE_MODIFIERS, ItemAttributeModifiers.EMPTY);
		double damage = modifiers.compute(Attributes.ATTACK_DAMAGE, BASE_ATTACK_DAMAGE, EquipmentSlot.MAINHAND);
		int level = enchantment(stack, sharpness);
		double bonus = level > 0 ? SHARPNESS_PER_LEVEL * level + SHARPNESS_PER_LEVEL : 0.0;
		return damage + bonus + condition(stack);
	}

	double miningSpeed(ItemStack stack, BlockState block, ResourceKey<Enchantment> efficiency) {
		float speed = stack.getDestroySpeed(block);
		int level = enchantment(stack, efficiency);
		double bonus = speed > 1.0F && level > 0 ? level * level + 1 : 0.0;
		return speed + bonus + condition(stack);
	}

	double enchanted(ItemStack stack, ResourceKey<Enchantment> enchantment) {
		return enchantment(stack, enchantment) + condition(stack);
	}

	static double count(ItemStack stack) {
		return stack.getCount();
	}

	static double buildingBlocks(ItemStack stack) {
		if (!(stack.getItem() instanceof BlockItem item)) {
			return UNMATCHED;
		}
		Block block = item.getBlock();
		BlockState state = block.defaultBlockState();
		boolean solid = state.isCollisionShapeFullBlock(EmptyBlockGetter.INSTANCE, BlockPos.ZERO) && !state.hasBlockEntity();
		return solid && !(block instanceof FallingBlock) ? stack.getCount() : UNMATCHED;
	}

	private int enchantment(ItemStack stack, ResourceKey<Enchantment> key) {
		return this.registries.lookupOrThrow(Registries.ENCHANTMENT)
			.get(key)
			.map(enchantment -> EnchantmentHelper.getItemEnchantmentLevel(enchantment, stack))
			.orElse(0);
	}

	private static double condition(ItemStack stack) {
		if (!stack.isDamageableItem()) {
			return WEAR_WEIGHT;
		}
		return WEAR_WEIGHT * (1.0 - (double) stack.getDamageValue() / stack.getMaxDamage());
	}
}
