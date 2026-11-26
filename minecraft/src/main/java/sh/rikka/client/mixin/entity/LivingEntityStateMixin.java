package sh.rikka.client.mixin.entity;

import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.world.entity.LivingEntity;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import sh.rikka.client.render.entity.RetainedEntityState;

@Mixin(LivingEntity.class)
abstract class LivingEntityStateMixin implements RetainedEntityState {
	@Unique
	private @Nullable EntityRenderState rikka$retainedState;

	@Unique
	private long rikka$retainedAt;

	@Override
	public @Nullable EntityRenderState rikka$retainedState() {
		return this.rikka$retainedState;
	}

	@Override
	public long rikka$retainedAt() {
		return this.rikka$retainedAt;
	}

	@Override
	public void rikka$retainState(EntityRenderState state, long nanos) {
		this.rikka$retainedState = state;
		this.rikka$retainedAt = nanos;
	}
}
