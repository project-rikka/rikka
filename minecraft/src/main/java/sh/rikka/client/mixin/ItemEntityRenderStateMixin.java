package sh.rikka.client.mixin;

import net.minecraft.client.renderer.entity.state.ItemEntityRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import sh.rikka.client.mods.item.DroppedItemState;

@Mixin(ItemEntityRenderState.class)
abstract class ItemEntityRenderStateMixin implements DroppedItemState {
	@Unique
	private float rikka$tumble;

	@Override
	public void rikka$setTumble(float degrees) {
		this.rikka$tumble = degrees;
	}

	@Override
	public float rikka$tumble() {
		return this.rikka$tumble;
	}
}
