package sh.rikka.client.mixin;

import net.minecraft.world.entity.item.ItemEntity;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import sh.rikka.client.mods.item.ItemTumble;
import sh.rikka.client.mods.item.TumblingItem;

@Mixin(ItemEntity.class)
abstract class ItemEntityMixin implements TumblingItem {
	@Unique
	private @Nullable ItemTumble rikka$tumble;

	@Override
	public ItemTumble rikka$tumble() {
		ItemTumble tumble = this.rikka$tumble;
		if (tumble == null) {
			tumble = new ItemTumble();
			this.rikka$tumble = tumble;
		}
		return tumble;
	}
}
