package sh.rikka.client.mods.animation;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.world.item.ItemDisplayContext;
import org.joml.Matrix4f;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mixin.ItemStackRenderStateAccessor;
import sh.rikka.client.mixin.LayerRenderStateAccessor;

record DisplayedItem(ItemFamily family, Matrix4f display) {
	static @Nullable DisplayedItem of(ItemStackRenderState state, ItemDisplayContext context) {
		ItemStackRenderStateAccessor accessor = (ItemStackRenderStateAccessor) state;
		int count = accessor.rikka$activeLayerCount();
		if (count == 0) {
			return null;
		}
		ItemStackRenderState.LayerRenderState[] layers = accessor.rikka$layers();
		LayerRenderStateAccessor first = (LayerRenderStateAccessor) layers[0];
		ItemFamily family = ItemFamily.recognize(context, first.rikka$itemTransform());
		if (family == null) {
			return null;
		}
		Matrix4f display = displayMatrix(first);
		for (int index = 1; index < count; index++) {
			if (!displayMatrix((LayerRenderStateAccessor) layers[index]).equals(display)) {
				return null;
			}
		}
		return new DisplayedItem(family, display);
	}

	private static Matrix4f displayMatrix(LayerRenderStateAccessor layer) {
		PoseStack.Pose pose = new PoseStack.Pose();
		layer.rikka$applyTransform(pose);
		return new Matrix4f(pose.pose());
	}
}
