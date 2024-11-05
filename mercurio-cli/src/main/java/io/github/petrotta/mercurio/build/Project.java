package io.github.petrotta.mercurio.build;

import org.eclipse.emf.ecore.resource.Resource;
import org.eclipse.emf.ecore.resource.ResourceSet;
import org.eclipse.xtext.validation.Issue;
import org.omg.sysml.interactive.SysMLInteractive;
import org.omg.sysml.interactive.VizResult;

import java.io.File;
import java.io.IOException;
import java.util.List;
import java.util.Set;

public abstract class Project {


    private final File library;
    private final File dir;
    private final SysMLInteractive interactive;

    public Project(File dir, File library) throws IOException {
        this(dir, library, false);
    }
    public Project(File dir, File library, boolean verbose) throws IOException {
        this.dir = dir;
        this.library = library;

        this.interactive = SysMLInteractive.createInstance();
        this.interactive.setVerbose(verbose);

        readLibrary();
    }

    protected File getDir() {
        return dir;
    }
    protected abstract File getSourceDir();

    public void readSysML() throws IOException {
        interactive.readAll(getSourceDir().toString(), true, ".sysml");
    }

    private void readLibrary() throws IOException {
        interactive.readAll(new File(library, "sysml.library/Kernel Libraries").toString(), false, ".kerml");
        interactive.readAll(new File(library, "sysml.library/Domain Libraries").toString(), false, ".sysml");
        interactive.readAll(new File(library, "sysml.library/Systems Libraries").toString(), false, ".kerml");

    }
    public String eval(String input, String target) {
        return interactive.eval(input, target);
    }

    public Set<Resource> getInputResources() {
        return interactive.getInputResources();
    }
    public ResourceSet getResourceSet() {
        return interactive.getResourceSet();
    }
    public List<Issue> validate() {
        return interactive.validate();
    }


    public VizResult viz(List<String> names, List<String> views, List<String> styles, List<String> help) {
        return  interactive.viz(names, views, styles, help);
    }




}
