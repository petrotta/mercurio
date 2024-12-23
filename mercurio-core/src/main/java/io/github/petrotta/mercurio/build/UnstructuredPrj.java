package io.github.petrotta.mercurio.build;

import java.io.File;
import java.io.IOException;

public class UnstructuredPrj extends Project {

    public UnstructuredPrj(File dir, File library) throws IOException {
        super(dir,library);
    }
    public UnstructuredPrj(File dir, File library, boolean verbose) throws IOException {
        super(dir, library, verbose);
    }

    @Override
    protected File getSourceDir() {
        return this.getDir();
    }
}
