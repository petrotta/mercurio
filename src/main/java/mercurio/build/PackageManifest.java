package mercurio.build;

import java.util.ArrayList;
import java.util.List;

public class PackageManifest {

    public void addDepend(Depend val) {
        depend.add(val);
    }
    public List<Depend> getDepend() {
        return depend;
    }

    public List<Depend> depend = new ArrayList<>();

    public String org;

    public String getDescription() {
        return description;
    }

    public void setDescription(String description) {
        this.description = description;
    }

    public String description;


    public String getOrg() {
        return org;
    }

    public void setOrg(String org) {
        this.org = org;
    }

    public String getProject() {
        return project;
    }

    public void setProject(String project) {
        this.project = project;
    }

    public String getVersion() {
        return version;
    }

    public void setVersion(String version) {
        this.version = version;
    }

    public String project;
    public String version;
}