package io.github.petrotta.mercurio.plugin;

import org.gradle.api.Plugin;
import org.gradle.api.Project;

import org.gradle.process.ExecResult;



public class GradlePlugin implements Plugin<Project> {


    @Override
    public void apply(Project project) {

        String group = "sysml";

        project.getConfigurations().create("main");



        project.getTasks().create("clean", task -> {
            task.setGroup(group);

            task.doLast(t -> {


                ExecResult result = project.exec(a-> {
                    a.setExecutable("mercurio");
                    a.args("clean");
                });
            });
        });

        project.getTasks().create("evaluate", task -> {
            task.setGroup(group);

            task.doLast(t -> {
                ExecResult result = project.exec(a-> {
                    a.setExecutable("mercurio");
                    a.args("evaluate");
                });
            });
        });

        project.getTasks().create("inspect", task -> {
            task.setGroup(group);

            task.doLast(t -> {
                ExecResult result = project.exec(a-> {
                    a.setExecutable("mercurio");
                    a.args("inspect");
                });
            });
        });

        project.getTasks().create("package", task -> {
            task.setGroup(group);

            task.doLast(t -> {
                ExecResult result = project.exec(a-> {
                    a.setExecutable("mercurio");
                    a.args("package");
                });
            });
        });


        project.getTasks().create("run", task -> {
            task.setGroup(group);

            task.doLast(t -> {
                ExecResult result = project.exec(a-> {
                    a.setExecutable("mercurio");
                    a.args("run");
                });
            });
        });

        project.getTasks().create("test", task -> {
            task.setGroup(group);

            task.doLast(t -> {
                ExecResult result = project.exec(a-> {
                    a.setExecutable("mercurio");
                    a.args("test");
                });
            });
        });

        project.getTasks().create("validate", task -> {
            task.setGroup(group);

            task.doLast(t -> {
//                try {
//                    GradleProject prj = Application.openGradleProject(project.getProjectDir(), null, true);
//                    prj.readSysML();
//
//
//                    console("Resources read: " + prj.getResourceSet().getResources().size());
//
//                    List<Issue> issues = prj.validate();
//                    console("Total issues: " + issues.size());
//                    for(Issue issue : issues) {
//                        console(issue.toString());
//                    }
//
//
//                } catch (Exception e) {
//                    throw new RuntimeException(e);
//                }

            });
        });

        project.getTasks().create("version", task -> {
            task.setGroup(group);

            task.doLast(t -> {
                ExecResult result = project.exec(a-> {
                    a.setExecutable("mercurio");
                    a.args("version");
                });
            });
        });


    }
}
