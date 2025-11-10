# Release guidelines

---

1. Create a branch from the `main` branch called `vMAJOR.MINOR`
2. Run the [Release](https://github.com/mhytrek/Signal_client/actions/workflows/release.yml) workflow from the created branch.
    - In the *Version* field enter the version in `MAJOR.MINOR.PATCH` format.
    - If the created version is a prerelease check the *Prerelease* checkbox, and name the version accordingly,
      for example `MAJOR.MINOR-rc0`
3. In case new fixes must be backported to create the patch release, they have to be backported to created release branch.
