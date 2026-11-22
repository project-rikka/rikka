export const site = {
  name: "Rikka",
  url: "https://rikka.sh",
  description:
    "Wayland compositor and desktop on NixOS, built for clean UI and usability.",
  repository: "https://github.com/project-rikka/rikka",
  discord: "https://discord.gg/m39vzmyg5J",
  install: "curl -fsSL https://install.rikka.sh | sudo sh",
  existingNixos:
    "https://github.com/project-rikka/rikka/blob/main/docs/en/installation.md#add-rikka-to-an-existing-nixos",
} as const;

export const mascot = {
  alt: "Rikka, the project mascot: a girl with long silver-blue hair and a snowflake hair clip, in a white coat and skirt.",
  repository: "https://github.com/project-rikka/rikka-chan",
} as const;
