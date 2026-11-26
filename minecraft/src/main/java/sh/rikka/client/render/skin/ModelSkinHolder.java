package sh.rikka.client.render.skin;

import org.jspecify.annotations.Nullable;

public interface ModelSkinHolder {
	@Nullable ModelSkin rikka$skin(int generation);

	void rikka$setSkin(ModelSkin skin, int generation);
}
