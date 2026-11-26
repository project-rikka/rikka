package sh.rikka.client.render.culling;

import net.minecraft.client.renderer.blockentity.BlockEntityRenderer;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.phys.Vec3;

final class RendererTraits {
	private static final ClassValue<Boolean> ENTITY_REACH_OVERRIDDEN = new ClassValue<>() {
		@Override
		protected Boolean computeValue(Class<?> type) {
			return overrides(type, EntityRenderer.class, "shouldRender", Entity.class, Frustum.class, double.class, double.class, double.class, float.class);
		}
	};
	private static final ClassValue<Boolean> BLOCK_ENTITY_REACH_OVERRIDDEN = new ClassValue<>() {
		@Override
		protected Boolean computeValue(Class<?> type) {
			return overrides(type, BlockEntityRenderer.class, "shouldRender", BlockEntity.class, Vec3.class)
				|| overrides(type, BlockEntityRenderer.class, "getViewDistance")
				|| overrides(type, BlockEntityRenderer.class, "shouldRenderOffScreen");
		}
	};

	private RendererTraits() {
	}

	static boolean extendsEntityReach(EntityRenderer<?, ?> renderer) {
		return ENTITY_REACH_OVERRIDDEN.get(renderer.getClass());
	}

	static boolean extendsBlockEntityReach(BlockEntityRenderer<?, ?> renderer) {
		return BLOCK_ENTITY_REACH_OVERRIDDEN.get(renderer.getClass());
	}

	private static boolean overrides(Class<?> type, Class<?> base, String name, Class<?>... parameters) {
		try {
			return type.getMethod(name, parameters).getDeclaringClass() != base;
		} catch (NoSuchMethodException missing) {
			throw new IllegalStateException("renderer method " + base.getName() + "." + name + " is missing", missing);
		}
	}
}
