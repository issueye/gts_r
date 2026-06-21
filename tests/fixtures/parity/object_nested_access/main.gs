let doc = { user: { name: "ada", score: 7 } };
doc.user.score = doc.user.score + 5;

println(`object-nested-access=${doc.user.name}:${doc.user.score}`);
