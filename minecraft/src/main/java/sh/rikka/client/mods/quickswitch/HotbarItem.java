package sh.rikka.client.mods.quickswitch;

import java.util.Locale;
import net.minecraft.tags.ItemTags;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.Enchantments;
import net.minecraft.world.level.block.Blocks;

enum HotbarItem {
	SWORD((stack, scores) -> stack.is(ItemTags.SWORDS) ? scores.attackDamage(stack, Enchantments.SHARPNESS) : ItemScores.UNMATCHED),
	AXE((stack, scores) -> stack.is(ItemTags.AXES)
		? scores.miningSpeed(stack, Blocks.OAK_LOG.defaultBlockState(), Enchantments.EFFICIENCY)
		: ItemScores.UNMATCHED),
	PICKAXE((stack, scores) -> stack.is(ItemTags.PICKAXES)
		? scores.miningSpeed(stack, Blocks.STONE.defaultBlockState(), Enchantments.EFFICIENCY)
		: ItemScores.UNMATCHED),
	SHOVEL((stack, scores) -> stack.is(ItemTags.SHOVELS)
		? scores.miningSpeed(stack, Blocks.DIRT.defaultBlockState(), Enchantments.EFFICIENCY)
		: ItemScores.UNMATCHED),
	BOW((stack, scores) -> stack.is(Items.BOW) ? scores.enchanted(stack, Enchantments.POWER) : ItemScores.UNMATCHED),
	FISHING_ROD((stack, scores) -> stack.is(Items.FISHING_ROD) ? scores.enchanted(stack, Enchantments.LURE) : ItemScores.UNMATCHED),
	BLOCKS((stack, scores) -> ItemScores.buildingBlocks(stack)),
	ENDER_PEARL((stack, scores) -> stack.is(Items.ENDER_PEARL) ? ItemScores.count(stack) : ItemScores.UNMATCHED),
	GOLDEN_APPLE((stack, scores) -> stack.is(Items.GOLDEN_APPLE) ? ItemScores.count(stack) : ItemScores.UNMATCHED);

	private final Scorer scorer;
	private final String id = name().toLowerCase(Locale.ROOT);

	HotbarItem(Scorer scorer) {
		this.scorer = scorer;
	}

	String id() {
		return this.id;
	}

	double score(ItemStack stack, ItemScores scores) {
		return stack.isEmpty() ? ItemScores.UNMATCHED : this.scorer.score(stack, scores);
	}

	@FunctionalInterface
	private interface Scorer {
		double score(ItemStack stack, ItemScores scores);
	}
}
