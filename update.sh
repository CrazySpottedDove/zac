#!/bin/bash
version=$1
cargo build --release --features pb --target-dir target_linux_pb --bin zacpb
cargo build --release --target-dir target_linux_no_pb --bin zac
cargo build --release --features pb --target-dir target_windows_pb --target x86_64-pc-windows-gnu --bin zacpb
cargo build --release --target-dir target_windows_no_pb --target x86_64-pc-windows-gnu --bin zac

if [ -z "$version" ]; then
    echo "Usage: $0 <version eg. v0.0.1> <comment>"
    exit 1
fi

comment=$2
if [ -z "$comment" ]; then
    echo "Usage: $0 <version eg. v0.0.1> <comment>"
    exit 1
fi

git add .
git commit -m "Release $version: $comment"
git tag "$version"
git push origin master "$version" --force

# linux_no_pb_path=$(pwd)/target_linux_no_pb/release/zac
# linux_pb_path=$(pwd)/target_linux_pb/release/zacpb
# windows_no_pb_path=$(pwd)/target_windows_no_pb/x86_64-pc-windows-gnu/release/zac.exe
# windows_pb_path=$(pwd)/target_windows_pb/x86_64-pc-windows-gnu/release/zacpb.exe

# gh release create "${version}" "${linux_no_pb_path}" "${windows_no_pb_path}" "${linux_pb_path}" "${windows_pb_path}" --title "${version}" --latest --notes "无进度条版本<br>**linux** : zac <br> **windows** : zac.exe <br>有进度条版本<br>**linux** : zacpb <br> **windows** : zacpb.exe"

#  The script is pretty simple. It takes two arguments: the version and the comment. It then commits the changes, tags the commit with the version, pushes the changes to the remote repository, and creates a new release on GitHub.
#  The script assumes that the binary files are located in the  target/release  and  target/x86_64-pc-windows-gnu/release  directories.
#  To run the script, you can use the following command:
#  bash update.sh v0.0.1 "Initial release"

#  This will create a new release on GitHub with the tag  v0.0.1  and the comment  Initial release .
#  Conclusion
#  In this article, we have seen how to create a simple script to automate the release process of a Rust project. We have used the  git  and  gh  commands to commit the changes, tag the commit, push the changes to the remote repository, and create a new release on GitHub.
#  The script can be further improved by adding more features like checking if the version is valid, checking if the tag already exists, and adding more error handling.
#  I hope you found this article helpful. If you have any questions or feedback, feel free to leave a comment.


