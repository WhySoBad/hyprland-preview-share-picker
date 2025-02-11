# TODO's

- [ ] Make protocol implementation smoother -> at the moment the frame/output managers can only
      process one requests sequentially which is an artifical limitation (use `RefCell` as data)

- [x] Support restore tokens (also support `allow_token_by_default` xdph config option)

- [x] Change version command to use a build env which contains the git version (set in `build.rs`)

- [ ] Cache compiled SCSS stylesheets to fasten startup

- [ ] Configurable default tab