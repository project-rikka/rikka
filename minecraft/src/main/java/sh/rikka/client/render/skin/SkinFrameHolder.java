package sh.rikka.client.render.skin;

import org.jspecify.annotations.Nullable;

public interface SkinFrameHolder {
	SkinFrame rikka$skinFrame();

	@Nullable SkinFrame rikka$openSkinFrame();

	SkinGroup rikka$buildingGroup();

	void rikka$setBuildingGroup(SkinGroup group);
}
