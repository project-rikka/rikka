package sh.rikka.client.menu;

import os.rikka.Rikka;
import os.rikka.TextRole;
import os.rikka.View;

final class Heading {
	private Heading() {
	}

	static void view(View view, String text) {
		view.text(text).textStyle(TextRole.HEADING).marginEdges(Rikka.SPACE_SNUG, 0.0F, 0.0F, Rikka.SPACE_TIGHT);
	}
}
