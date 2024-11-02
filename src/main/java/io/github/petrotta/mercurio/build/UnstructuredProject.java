package io.github.petrotta.mercurio.build;

import java.io.File;
import java.io.IOException;

public class UnstructuredProject extends Project {

    public UnstructuredProject(File dir, File library) throws IOException {
        super(dir,library);
    }
    public UnstructuredProject(File dir, File library, boolean verbose) throws IOException {
        super(dir, library, verbose);
    }

    @Override
    protected File getSourceDir() {
        return this.getDir();
    }
}
