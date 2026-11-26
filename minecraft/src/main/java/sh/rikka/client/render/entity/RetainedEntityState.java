package sh.rikka.client.render.entity;

import net.minecraft.client.renderer.entity.state.EntityRenderState;
import org.jspecify.annotations.Nullable;

public interface RetainedEntityState {
	@Nullable EntityRenderState rikka$retainedState();

	long rikka$retainedAt();

	void rikka$retainState(EntityRenderState state, long nanos);
}
