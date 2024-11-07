package io.github.petrotta.mercurio.plugins;

import io.github.petrotta.mercurio.build.Project;

public abstract class Plugin {
    public abstract void init();
    public abstract void run(Project project);
}
