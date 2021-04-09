const socket = new WebSocket('ws://localhost:8008')

const send = msg => socket.send(JSON.stringify(msg))

const avatars = [
    '14571e81fa5ce366eb09d6b3cbadea53',
    '2894_Aww_Blob_DarkerEyes',
    '6876_BlobCatLove',
    'ablobspin',
    'amaze',
    'babaleine',
    'bisexual_flag',
    'blobcatlost',
    'blobcatmeltcry',
    'blobcatpeek',
    'blobcatrainbow',
    'blobfingerguns',
    'bloblight',
    'blobraccoon',
    'blobsweat',
    'bon_anniversaire',
    'bonnaum',
    'bzh',
    'camille',
    'ca_pulse',
    'cerbere',
    'chat_jpg',
    'chocolatine',
    'cornichouette',
    'dehornoy',
    'derp',
    'eopeek',
    'excuse',
    'ferrispeek',
    'fogogoh-ananas',
    'fogogoh-grr',
    'fogogoh-peek',
    'fogogoh',
    'grrnoble',
    'grr_peek',
    'grr',
    'grrscute',
    'hhhhaaAAAAHHHH',
    'hyperthink',
    'johanpakontan',
    'JPEG_20191004_194401.jpg',
    'maaaarrkooovicczzzzz',
    'markoccinelle',
    'marko_joy',
    'marko',
    'marko_terminator',
    'matteo',
    'muannob',
    'ohno',
    'oof',
    'pajojo',
    'pa_kontan',
    'pierrotbis',
    'polytech',
    'rustacean-flat-happy-smol',
    'steven_pun',
    'tabouret',
    'thaenkin',
    'theo_sepulcre',
    'thinkin',
    'tinking',
    'transgender_flag',
    'unsafe',
    'vero'
]

// State
let game = 'CACA'
let players = [ 'TIBO', 'Analogie' ]
let question = 'Qui est un prout ?'
let avatar = avatars[0]

socket.addEventListener('message', event => {
    const data = JSON.parse(event.data)
    switch (data.tag) {
        case 'RoomCreated':
            game = data.code
            send({
                tag: 'JoinRoom',
                username: document.getElementById('username').value,
                avatar: avatar,
                code: game
            })
            break
        case 'OnRoomJoin':
            game = data.code
            players = data.players
            m.redraw()
            break
        case 'RoomUpdate':
            players = data.players
            m.redraw()
            break
        case 'NewRound':
        case 'GameStart':
            question = data.question
            m.route.set('/question')
            break
        default:
            console.log('Unknown message', data)
            break
    }
})

const Home = {
    view: () => {
        return m('main', {}, [
            m('section', {}, [
                m('h2', {}, 'T ki'),
                m('p', {}, 'Entre ton nom et choisi ton avatar.'),
                m('input', { id: 'username' }),
                m('div', { id: 'avatar-selector' }, avatars.map(a =>
                    m('img', {
                        id: a,
                        className: a == avatar ? 'selected' : '',
                        src: `/static/avatars/${a}.png`,
                        onclick: _ => {
                            document.getElementById(avatar).classList.remove('selected')
                            avatar = a
                            document.getElementById(avatar).classList.add('selected')
                        }
                    })  
                ))
            ]),
            m('section', {}, [
                m('h2', {}, 'Créer une partie'),
                m('p', {}, 'Creéz une partie depuis un ordinateur (ou autre grand écran que tout le monde peut voir). Vous pourrez ensuite la rejoindre avec vos téléphones.'),
                m('a', { className: 'button', onclick: () => {
                    send({
                        tag: 'CreateRoom'
                    })
                } }, 'Créer une partie')
            ]),
            m('section', {}, [
                m('h2', {}, 'Rejoindre une partie'),
                m('p', {}, 'Entrez le code de la partie qui s\'affiche sur le grand écran.'),
                m('input', { id: 'code' }),
                m('a', { className: 'button', href: '#', onclick: () => {
                    send({
                        avatar: avatar,
                        username: document.getElementById('username').value,
                        code: document.getElementById('code').value
                    })
                } }, 'Rejoindre')
            ])
        ])
    }
}

const Lobby = {
    view: () => m('main', {}, [
        m('h2', {}, game),
        m('section', { className: 'lobby' },
            players.map(x => m('div', {}, [
                m('img', { className: 'avatar' }),
                m('p', {}, x),
            ]))
        )
    ])
}

const Question = {
    view: () => m('main', {}, [
        m('h2', {}, question),
        m('section', {}, players.map(x => m('a', { className: 'button' }, x)))
    ])
}

const Result = {
    view: () => m('main', {}, [
        m('h2', {}, 'Résultats'),
        m('h3', {}, question),
        m('section', {}, answers.map(a =>
            m('div', {}, [
                m('img', { src: a.avatar }),
                m('p', {}, a.username),
                m('p', {}, `${a.votes} votes`)
            ])
        ))
    ])
}

m.route(document.getElementById('app'), '/home', {
    '/home': Home,
    '/lobby': Lobby,
    '/question': Question,
    '/result': Result,
})